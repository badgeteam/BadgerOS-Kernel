// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{mem::offset_of, sync::atomic::AtomicU32};

use super::*;

use crate::{
    bindings::raw::timestamp_us_t,
    dev2::bus::ata::AtaBus,
    kernel::{
        sched::thread_yield,
        sync::{semaphore::Semaphore, spinlock::RawSpinlock, waitlist::Waitlist},
    },
    mem::pmm::phys_box::PhysBox,
};

/// Number of commands per port.
const CMDLIST_LEN: usize = 4;
/// Size per command table.
const CMD_TABLE_SIZE: usize = 0x200;
/// Scatter-gather list size per port.
const PRDT_LIST_LEN: usize = (CMD_TABLE_SIZE - 0x80) / size_of::<hms::PRDT>();

/// One command table.
struct CmdTable {
    cmd_fis: fis::CmdFis,
    atapi_cmd: [u8; 0x10],
    _resvd0: [u8; 0x30],
    prdt: [hms::PRDT; PRDT_LIST_LEN],
}
static_assertions::assert_eq_size!(CmdTable, [u8; CMD_TABLE_SIZE]);

/// Host memory structures per port.
struct PortHms {
    cmd_tables: [CmdTable; CMDLIST_LEN],
    cmdlist: [hms::CmdHdr; CMDLIST_LEN],
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
    /// Spinlock that guards the interrupt enable register.
    irqen_lock: RawSpinlock,
    /// Host memory structures.
    hms: PhysBox<PortHms>,
}

impl Port {
    pub(super) fn new(index: u8, reg: &reg::Port, cmdlist_len: usize) -> EResult<Self> {
        debug_assert!(index < 32);
        let hms = unsafe { PhysBox::<PortHms>::try_new(false, true)? };
        let cmdlist_len = cmdlist_len.min(CMDLIST_LEN);

        for i in 0..CMDLIST_LEN {
            let table_paddr =
                hms.paddr() + offset_of!(PortHms, cmd_tables) + i * size_of::<hms::CmdHdr>();
            hms.cmdlist[i].cmd_addr_hi.set((table_paddr >> 32) as u32);
            hms.cmdlist[i].cmd_addr_lo.set(table_paddr as u32);
        }

        let cmdlist_paddr = hms.paddr() + offset_of!(PortHms, cmdlist);
        reg.cmdlist_addr_hi.set((cmdlist_paddr >> 32) as u32);
        reg.cmdlist_addr_lo.set(cmdlist_paddr as u32);

        let rfis_paddr = hms.paddr() + offset_of!(PortHms, rfis);
        reg.fis_addr_hi.set((rfis_paddr >> 32) as u32);
        reg.fis_addr_lo.set(rfis_paddr as u32);

        Ok(Self {
            work_waitlist: Waitlist::new(),
            cmd_waitlist: [const { Waitlist::new() }; _],
            index,
            state: AtomicU32::new(0),
            cmd_avail_count: Semaphore::with_count(cmdlist_len as u32),
            cmd_avail_map: AtomicU32::new((1u32 << cmdlist_len).wrapping_neg()),
            cmd_issue_map: AtomicU32::new(0),
            cmd_finish_map: AtomicU32::new(0),
            cmd_err_map: AtomicU32::new(0),
            irqen_lock: RawSpinlock::new(),
            hms,
        })
    }

    /// Create and start the driver thread.
    pub(super) fn start(&self, dev: Arc<SataAhciCtrl>) -> EResult<()> {
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
    fn thread_main(&self, dev: Arc<SataAhciCtrl>) {
        let reg = &dev.reg.port[self.index as usize];

        Self::restart_port(reg);

        let mut inflight_map = 0;
        while self.state.load(Ordering::Relaxed) == 1 {
            // Wait for something to need attention.
            self.work_waitlist.unintr_block(timestamp_us_t::MAX, || {
                if self.cmd_issue_map.load(Ordering::Relaxed) != 0 {
                    return true;
                }

                let stat = reg.irq_status.get();

                reg::PortIrq::tf_err.is_set(stat)
                    || reg::PortIrq::hb_fatal_err.is_set(stat)
                    || reg::PortIrq::hb_data_err.is_set(stat)
                    || reg::PortIrq::if_fatal.is_set(stat)
            });

            let stat = reg.irq_status.get();

            // Check for fatal error interrupts.
            if reg::PortIrq::tf_err.is_set(stat)
                || reg::PortIrq::hb_fatal_err.is_set(stat)
                || reg::PortIrq::hb_data_err.is_set(stat)
                || reg::PortIrq::if_fatal.is_set(stat)
            {
                // Command caused a fatal error.
                let culprit = reg.cmd.read(reg::PortCmd::cur_slot) as usize;
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

            // We're interested in all fatal errors, and command completions.
            // We can tell commands are completed by the D2H register FIS, SDB FIS and the completion of the final PRD for DMA accesses.
            {
                let _guard = self.irqen_lock.lock();
                reg.irq_enable.write(
                    reg::PortIrq::tf_err::SET
                        + reg::PortIrq::hb_fatal_err::SET
                        + reg::PortIrq::hb_data_err::SET
                        + reg::PortIrq::if_fatal::SET
                        + reg::PortIrq::set_dev_bits::SET
                        + reg::PortIrq::d2h_reg_fis::SET
                        + reg::PortIrq::prd_proc::SET,
                );
            }
        }

        self.state
            .compare_exchange(2, 0, Ordering::Relaxed, Ordering::Relaxed)
            .expect("AHCI port thread stopped twice");
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
    pub(super) fn interrupt(&self, ctrl: &SataAhciCtrl) {
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
        let tmp = self.cmd_avail_map.fetch_or(mask, Ordering::Relaxed);
        debug_assert!(tmp & mask == 0, "Command list issued twice");

        loop {
            self.cmd_waitlist[list].unintr_block(timestamp_us_t::MAX, || {
                (self.cmd_finish_map.load(Ordering::Relaxed)
                    | self.cmd_err_map.load(Ordering::Relaxed))
                    & mask
                    != 0
            });

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
        }
    }

    pub fn ata_cmd(
        &self,
        cmd: Command,
        ctrl: u8,
        sec_count: u16,
        feature: u16,
        lba: u64,
        data: Option<MaybeMut<'_, [u8]>>,
    ) -> EResult<()> {
        todo!()
    }
}
