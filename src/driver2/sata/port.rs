// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{mem::offset_of, sync::atomic::AtomicU32};

use super::*;

use crate::{
    badgelib::irq::IrqGuard,
    bindings::raw::timestamp_us_t,
    dev2::{bus::ata::AtaBus, registry},
    kernel::{
        sched::{Thread, thread_yield},
        sync::{semaphore::Semaphore, spinlock::RawSpinlock, waitlist::Waitlist},
    },
    mem::{dma::DmaTarget, pmm::phys_box::PhysBox},
};

/// Number of commands per port.
const CMDLIST_LEN: usize = 4;
/// Size per command table.
const CMD_TABLE_SIZE: usize = 0x200;
/// Scatter-gather list size per port.
const PRDT_LIST_LEN: usize = (CMD_TABLE_SIZE - 0x80) / size_of::<hms::PRDT>();

/// One command table.
#[repr(C, align(0x80))]
struct CmdTable {
    cmd_fis: fis::CmdFis,
    atapi_cmd: [u8; 0x10],
    _resvd0: [u8; 0x30],
    prdt: [hms::PRDT; PRDT_LIST_LEN],
}
static_assertions::assert_eq_size!(CmdTable, [u8; CMD_TABLE_SIZE]);

/// Host memory structures per port.
#[repr(C, align(0x400))]
struct PortHms {
    // DO NOT REORDER: cmdlist needs to be sufficiently aligned.
    cmdlist: [hms::CmdHdr; CMDLIST_LEN],
    cmd_tables: [CmdTable; CMDLIST_LEN],
    rfis: fis::Received,
}

/// Per-port driver context for [`SataAhciCtrl`].
pub(super) struct Port {
    /// Waitlist for work to run on the driver thread.
    work_waitlist: Waitlist,
    /// Waitlist for command completion.
    cmd_waitlist: [Waitlist; CMDLIST_LEN],
    /// Port index.
    index: u8,
    /// Current state of the driver thread:
    /// 0: Thread isn't running.
    /// 1: Thread is running.
    /// 2: Thread is stopping.
    state: AtomicU32,
    /// Semaphore posted for each command list that is available.
    cmd_avail_count: Semaphore,
    /// Bitmask of which command lists are currently in use.
    cmd_avail_map: AtomicU32,
    /// Commands issued per slot.
    cmd_issue_map: AtomicU32,
    /// Command complete per slot.
    cmd_finish_map: AtomicU32,
    /// Command error per slot.
    cmd_err_map: AtomicU32,
    /// One or more commands has timed out.
    cmd_timeout: AtomicU32,
    /// Spinlock that guards the interrupt enable register.
    irqen_lock: RawSpinlock,
    /// Host memory structures.
    hms: PhysBox<PortHms>,
}

impl Port {
    pub(super) fn new(
        index: u8,
        supports_ss: bool,
        reg: &reg::Port,
        cmdlist_len: usize,
    ) -> EResult<Self> {
        debug_assert!(index < 32);
        let hms = unsafe { PhysBox::<PortHms>::try_new(false, true)? };
        let cmdlist_len = cmdlist_len.min(CMDLIST_LEN);

        for i in 0..CMDLIST_LEN {
            let table_paddr =
                hms.paddr() + offset_of!(PortHms, cmd_tables) + i * size_of::<CmdTable>();
            hms.cmdlist[i].cmd_addr_hi.set((table_paddr >> 32) as u32);
            hms.cmdlist[i].cmd_addr_lo.set(table_paddr as u32);
        }

        // Power on the port.
        if supports_ss {
            reg.cmd.modify(reg::PortCmd::spinup.val(1));
        }
        reg.cmd.modify(reg::PortCmd::if_comm_ctrl::ACTIVE);

        // Stop DMA engine.
        reg.cmd.modify(reg::PortCmd::cmd_start::CLEAR);
        let lim = time_us() + 100000;
        while reg.cmd.read(reg::PortCmd::cmd_running) != 0 {
            if time_us() > lim {
                return Err(Errno::ENAVAIL);
            }
            let _ = thread_sleep(5000);
        }
        reg.cmd.modify(reg::PortCmd::fis_en::CLEAR);
        let lim = time_us() + 100000;
        while reg.cmd.read(reg::PortCmd::fis_running) != 0 {
            if time_us() > lim {
                return Err(Errno::ENAVAIL);
            }
            let _ = thread_sleep(5000);
        }

        let cmdlist_paddr = hms.paddr() + offset_of!(PortHms, cmdlist);
        reg.cmdlist_addr_hi.set((cmdlist_paddr >> 32) as u32);
        reg.cmdlist_addr_lo.set(cmdlist_paddr as u32);

        let rfis_paddr = hms.paddr() + offset_of!(PortHms, rfis);
        reg.fis_addr_hi.set((rfis_paddr >> 32) as u32);
        reg.fis_addr_lo.set(rfis_paddr as u32);

        reg.cmd.modify(reg::PortCmd::fis_en::SET);

        Ok(Self {
            work_waitlist: Waitlist::new(),
            cmd_waitlist: [const { Waitlist::new() }; _],
            index,
            state: AtomicU32::new(0),
            cmd_avail_count: Semaphore::with_count(cmdlist_len as u32),
            cmd_avail_map: AtomicU32::new((1u32 << cmdlist_len) - 1),
            cmd_issue_map: AtomicU32::new(0),
            cmd_finish_map: AtomicU32::new(0),
            cmd_err_map: AtomicU32::new(0),
            cmd_timeout: AtomicU32::new(0),
            irqen_lock: RawSpinlock::new(),
            hms,
        })
    }

    /// Create and start the driver thread.
    pub(super) fn start(&self, dev: Arc<SataAhciCtl>) -> EResult<()> {
        self.state
            .compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed)
            .expect("AHCI port thread started twice");
        let index = self.index as usize;
        debug_assert!(core::ptr::addr_eq(self, dev.port[index].as_ref().unwrap()));

        let name = format!("{} port {}", &dev.bus, self.index);
        Thread::new(
            move || dev.port[index].as_ref().unwrap().thread_main(dev.clone()),
            None,
            Some(name),
        )?;

        Ok(())
    }

    /// Thread that handles interrupts on this port.
    fn thread_main(&self, dev: Arc<SataAhciCtl>) {
        let reg = &dev.reg.port[self.index as usize];

        Self::restart_port(reg);

        let bus = Arc::new(AtaBus::new(dev.clone(), self.index as u32));
        let mut is_registered = false;
        Self::check_conn(&mut is_registered, reg, &bus);

        let mut inflight_map = 0;
        while self.state.load(Ordering::Relaxed) == 1 {
            {
                let _noirq = IrqGuard::new();
                let _guard = self.irqen_lock.lock();
                reg.irq_enable.set(u32::MAX);
            }

            // Wait for something to need attention.
            self.work_waitlist.unintr_block(timestamp_us_t::MAX, || {
                if self.cmd_issue_map.load(Ordering::Relaxed) != 0 {
                    return false;
                }

                reg.irq_status.get() == 0
            });

            let stat = reg.irq_status.get();

            // Check for connection status changes.
            if reg::PortIrq::cold_status.is_set(stat)
                || reg::PortIrq::phy_rdy.is_set(stat)
                || reg::PortIrq::port_status.is_set(stat)
            {
                logkf!(
                    LogLevel::Debug,
                    "port {}: connection status changed",
                    self.index
                );
                Self::check_conn(&mut is_registered, reg, &bus);
            }

            // Check for commands timed out.
            if self
                .cmd_timeout
                .compare_exchange(1, 2, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                Self::restart_port(reg);

                while self.cmd_err_map.load(Ordering::Relaxed) != 0
                    || self.cmd_finish_map.load(Ordering::Relaxed) != 0
                {
                    thread_yield();
                }

                self.cmd_timeout.store(0, Ordering::Relaxed);

                continue;
            }

            // Check for fatal error interrupts.
            if reg::PortIrq::tf_err.is_set(stat)
                || reg::PortIrq::hb_fatal_err.is_set(stat)
                || reg::PortIrq::hb_data_err.is_set(stat)
                || reg::PortIrq::if_fatal.is_set(stat)
            {
                // Command caused a fatal error.
                let culprit = reg.cmd.read(reg::PortCmd::cur_slot) as usize;
                logkf!(
                    LogLevel::Error,
                    "{}: fatal error, culprit slot={}, irq_status=0x{:x}",
                    &bus,
                    culprit,
                    stat
                );
                // Put unprocessed commands back in the queue.
                let unproc_map = reg.cmd_issue.get() & !(1 << culprit);
                self.cmd_issue_map.fetch_or(unproc_map, Ordering::Relaxed);
                // Notify command failure to user.
                self.cmd_err_map.fetch_or(1 << culprit, Ordering::Relaxed);
                self.cmd_waitlist[culprit].notify();

                Self::restart_port(reg);
                continue;
            }

            // Check for commands that have completed.
            let unproc_map = reg.cmd_issue.get();
            let mut finished_map = inflight_map & !unproc_map;
            inflight_map &= !finished_map;
            self.cmd_finish_map
                .fetch_or(finished_map, Ordering::Relaxed);
            while finished_map != 0 {
                let index = finished_map.trailing_zeros() as usize;
                self.cmd_waitlist[index].notify();
                finished_map &= !(1 << index);
            }

            // Issue new commands.
            let issue_map = self.cmd_issue_map.swap(0, Ordering::Relaxed);
            inflight_map |= issue_map;
            reg.cmd_issue.set(issue_map);

            // Clear processed interrupts.
            reg.irq_status.set(stat);
        }

        registry::remove_bus(&*bus);

        self.state
            .compare_exchange(2, 0, Ordering::Relaxed, Ordering::Relaxed)
            .expect("AHCI port thread stopped twice");
    }

    /// Check the connection status of the port and (un-)register the bus as appropriate.
    fn check_conn(is_registered: &mut bool, reg: &reg::Port, bus: &Arc<AtaBus>) {
        let detect = reg.sstatus.read(reg::PortSStatus::detect) == 3;
        if !detect && *is_registered {
            (&**bus as &dyn Bus).cancel_resv();
            registry::remove_bus(&**bus);
            *is_registered = false;
        } else if detect && !*is_registered {
            if registry::register_bus(bus.clone()).is_ok() {
                logkf!(LogLevel::Info, "Added {} as bus {}", &bus, bus.id());
            }
            *is_registered = true;
        }
    }

    /// (Re-)start the AHCI port.
    fn restart_port(reg: &reg::Port) {
        reg.cmd.modify(reg::PortCmd::cmd_start::CLEAR);

        for _ in 0..50 {
            if !reg.cmd.is_set(reg::PortCmd::cmd_running) {
                break;
            }
            thread_yield();
        }
        while reg.cmd.is_set(reg::PortCmd::cmd_running) {
            let _ = thread_sleep(5000);
        }

        reg.irq_status.set(u32::MAX);
        reg.cmd.modify(reg::PortCmd::cmd_start::SET);
    }

    /// Stop the driver thread.
    pub(super) fn stop(&self) {
        let _ = self
            .state
            .compare_exchange(1, 2, Ordering::Relaxed, Ordering::Relaxed);
    }

    /// Signal an interrupt to the driver thread.
    pub(super) fn interrupt(&self, ctrl: &SataAhciCtl) {
        let _guard = self.irqen_lock.lock();
        let reg = &ctrl.reg.port[self.index as usize];

        let pending = reg.irq_status.get();
        reg.irq_enable.set(reg.irq_enable.get() & !pending);

        self.work_waitlist.notify();
    }

    /// Wait for and reserve a command list.
    fn cmd_start(&self) -> usize {
        self.cmd_avail_count.unintr_wait();
        loop {
            let cmd_avail_map = self.cmd_avail_map.load(Ordering::Relaxed);
            if cmd_avail_map == 0 {
                continue;
            }
            let index = cmd_avail_map.trailing_zeros() as usize;
            if self
                .cmd_avail_map
                .compare_exchange(
                    cmd_avail_map,
                    cmd_avail_map & !(1 << index),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return index;
            }
        }
    }

    /// Release a command list.
    fn cmd_cancel(&self, list: usize) {
        debug_assert!(list < CMDLIST_LEN);
        let tmp = self.cmd_avail_map.fetch_or(1 << list, Ordering::Relaxed);
        debug_assert!(tmp & (1 << list) == 0, "Command list released twice");
        self.cmd_avail_count.post();
    }

    /// Issue a command list and await its completion or error.
    fn cmd_issue(&self, list: usize) -> EResult<()> {
        debug_assert!(list < CMDLIST_LEN);
        let mask = 1u32 << list;
        let tmp = self.cmd_issue_map.fetch_or(mask, Ordering::Relaxed);
        debug_assert!(tmp & mask == 0, "Command list issued twice");
        self.work_waitlist.notify();

        let lim = time_us() + 100000;
        loop {
            if self.cmd_finish_map.load(Ordering::Relaxed) & mask != 0 {
                self.cmd_finish_map.fetch_and(!mask, Ordering::Relaxed);
                self.cmd_cancel(list);
                return Ok(());
            }

            if self.cmd_err_map.load(Ordering::Relaxed) & mask != 0 {
                self.cmd_err_map.fetch_and(!mask, Ordering::Relaxed);
                self.cmd_cancel(list);
                return Err(Errno::EIO);
            }

            let Some(timeout) = lim.checked_sub(time_us()) else {
                let _ =
                    self.cmd_timeout
                        .compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed);
                self.work_waitlist.notify();
                self.cmd_cancel(list);
                return Err(Errno::ETIMEDOUT);
            };

            self.cmd_waitlist[list].unintr_block(timeout as i64, || {
                (self.cmd_finish_map.load(Ordering::Relaxed)
                    | self.cmd_err_map.load(Ordering::Relaxed))
                    & mask
                    == 0
            });
        }
    }

    pub fn ata_cmd(
        &self,
        cmd: Command,
        ctrl: u8,
        sec_count: u16,
        feature: u16,
        lba: u64,
        data_offset: u64,
        data_length: u64,
        data: Option<&dyn DmaTarget>,
    ) -> EResult<()> {
        let list = self.cmd_start();
        let hdr = &self.hms.cmdlist[list];
        let fis = unsafe { &self.hms.cmd_tables[list].cmd_fis.register };
        let prdt = &self.hms.cmd_tables[list].prdt;

        // Collect scatter-gather list.
        if let Some(data) = data {
            let dma_size = data.size();
            if dma_size == 0 || dma_size & 1 != 0 || dma_size >= u32::MAX as u64 {
                logkf!(LogLevel::Error, "Invalid DMA size {} for AHCI", dma_size);
                return Err(Errno::EINVAL);
            }
            hdr.prd_len.set(dma_size as u32);

            let mut index = 0;
            data.collect(
                data_offset,
                data_length,
                u32::MAX as usize - 1,
                &mut |entry| {
                    debug_assert!(entry.size <= u32::MAX as usize - 1);
                    if index >= PRDT_LIST_LEN {
                        logkf!(LogLevel::Error, "Scatter-gather list too long");
                        return Err(Errno::ENOMEM);
                    }
                    if (entry.paddr | entry.vaddr) & 1 != 0 {
                        logkf!(LogLevel::Error, "Misaligned DMA buffer for AHCI");
                        return Err(Errno::EINVAL);
                    }

                    prdt[index].dbc.set((entry.size - 1) as u32);
                    prdt[index].paddr.set(entry.paddr as u64);
                    index += 1;

                    Ok(())
                },
            )?;
            debug_assert!(index > 0);

            prdt[index - 1].dbc.modify(hms::DBC::irq_en::SET);
            hdr.desc.write(
                hms::CmdHdrDesc::fis_len.val(size_of::<fis::RegisterH2D>() as u32 / 4)
                    + hms::CmdHdrDesc::prdtl.val(index as u32)
                    + hms::CmdHdrDesc::write.val(!data.allow_scatter() as u32)
                    + hms::CmdHdrDesc::clr_busy.val(1),
            );
        } else {
            hdr.desc.write(
                hms::CmdHdrDesc::fis_len.val(size_of::<fis::RegisterH2D>() as u32 / 4)
                    + hms::CmdHdrDesc::clr_busy.val(1),
            );
        }

        // Set of the H2D register FIS.
        let lba = lba.to_le_bytes();
        fis.lba[0].set(lba[0]);
        fis.lba[1].set(lba[1]);
        fis.lba[2].set(lba[2]);
        fis.lba_exp[0].set(lba[3]);
        fis.lba_exp[1].set(lba[4]);
        fis.lba_exp[2].set(lba[5]);
        fis.fis_type.set(fis::Type::RegisterH2D as u8);
        fis.command.set(cmd as u8);
        fis.device.set(0x40);
        fis.control.set(ctrl);
        fis.features.set(feature as u8);
        fis.features_exp.set((feature >> 8) as u8);
        fis.pmc.write(fis::PMC::cmdr_xfer.val(1));
        fis.sec_count.set(sec_count);

        self.cmd_issue(list)
    }
}
