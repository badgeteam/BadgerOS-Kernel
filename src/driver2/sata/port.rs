// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{mem::offset_of, sync::atomic::AtomicU32};

use super::*;

use crate::{
    kernel::sync::{semaphore::Semaphore, spinlock::RawSpinlock},
    mem::pmm::phys_box::PhysBox,
};

/// Number of commands per port.
const CMD_LIST_LEN: usize = 1;
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
    rfis: fis::Received,
    cmd_list: [hms::CmdHdr; CMD_LIST_LEN],
    cmd_table: [CmdTable; CMD_LIST_LEN],
}

/// Per-port driver context for [`SataAhciCtrl`].
pub(super) struct Port {
    /// Semaphore posted when an interrupt occurs.
    irq: Semaphore,
    /// Port index.
    index: u8,
    /// Current state of the driver thread:
    /// 0: Thread isn't running.
    /// 1: Thread is running.
    /// 2: Thread is stopping.
    state: AtomicU32,
    /// Spinlock that guards the port registers.
    reg_lock: RawSpinlock,
    /// Host memory structures.
    hms: PhysBox<PortHms>,
}

impl Port {
    pub(super) fn new(index: u8, reg: &reg::Port) -> EResult<Self> {
        debug_assert!(index < 32);
        let hms = unsafe { PhysBox::<PortHms>::try_new(false, true)? };

        let cmdlist_paddr = hms.paddr() + offset_of!(PortHms, cmd_list);
        reg.cmdlist_addr_hi.set((cmdlist_paddr >> 32) as u32);
        reg.cmdlist_addr_lo.set(cmdlist_paddr as u32);

        let rfis_paddr = hms.paddr() + offset_of!(PortHms, rfis);
        reg.fis_addr_hi.set((rfis_paddr >> 32) as u32);
        reg.fis_addr_lo.set(rfis_paddr as u32);

        Ok(Self {
            irq: Semaphore::new(),
            index,
            state: AtomicU32::new(0),
            reg_lock: RawSpinlock::new(),
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
            move || dev.port[index].as_ref().unwrap().thread_main(&dev),
            None,
            Some(name),
        )?;

        Ok(())
    }

    /// Thread main function.
    fn thread_main(&self, dev: &SataAhciCtrl) {
        while self.state.load(Ordering::Relaxed) == 1 {
            self.irq.unintr_wait();
        }

        self.state
            .compare_exchange(2, 0, Ordering::Relaxed, Ordering::Relaxed)
            .expect("AHCI port thread stopped twice");
    }

    /// Stop the driver thread.
    pub(super) fn stop(&self) {
        let _ = self
            .state
            .compare_exchange(1, 2, Ordering::Relaxed, Ordering::Relaxed);
    }

    pub(super) fn interrupt(&self, ctrl: &SataAhciCtrl) {
        let _guard = self.reg_lock.lock();
        let reg = &ctrl.reg.port[self.index as usize];

        let pending = reg.irq_status.get();
        reg.irq_enable.set(reg.irq_enable.get() & !pending);

        self.irq.post();
    }
}
