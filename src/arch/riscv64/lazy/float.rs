use core::arch::asm;

use crate::arch::riscv64::{csr::sstatus::*, except::RiscvExceptFrame};

#[derive(Clone, Copy, Default)]
pub struct RiscvLazyFloat {
    pub freg: [u64; 32],
    pub fcsr: u64,
}

impl RiscvLazyFloat {
    /// Load the state of the floating-point register file.
    pub fn load_state(&self, frame: &mut RiscvExceptFrame) {
        if (frame.sstatus & FS_MASK) >> FS_BIT == xs::OFF {
            return;
        }
        unsafe {
            asm!(
                "csrs sstatus, {xs_mask}
                .option push
                .option arch, +d
                .rept 32
                fld f\\+, \\+*8({self})
                .endr
                .option pop
                csrc sstatus, {xs_mask}",
                self = in(reg) self,
                xs_mask = in(reg) xs::MASK << FS_BIT,
            );
        }
        frame.sstatus &= !FS_MASK;
        frame.sstatus |= xs::CLEAN << FS_BIT;
    }

    /// Save the state of the floating-point register file.
    pub fn save_state(&mut self, frame: &mut RiscvExceptFrame) {
        if (frame.sstatus & FS_MASK) >> FS_BIT != xs::DIRTY {
            return; // No need to save clean state.
        }
        unsafe {
            asm!(
                "csrs sstatus, {xs_mask}
                .option push
                .option arch, +d
                csrr {fcsr}, fcsr
                .rept 32
                fsd f\\+, \\+*8({self})
                .endr
                .option pop
                csrc sstatus, {xs_mask}",
                fcsr = out(reg) self.fcsr,
                self = in(reg) self,
                xs_mask = in(reg) xs::MASK << FS_BIT,
            );
        }
        frame.sstatus &= !FS_MASK;
        frame.sstatus |= xs::CLEAN << FS_BIT;
    }

    /// Initially enable floating-point state.
    pub fn enable(&mut self, frame: &mut RiscvExceptFrame) -> bool {
        if frame.sstatus & FS_MASK >> FS_BIT != xs::OFF {
            return false; // Already enabled.
        }
        frame.sstatus |= xs::INIT << FS_BIT;
        unsafe {
            asm!(
                "csrs sstatus, {xs_mask}
                .option push
                .option arch, +d
                .rept 32
                fmv.d.x f\\+, x0
                .endr
                csrwi fcsr, 0
                .option pop
                csrc sstatus, {xs_mask}",
                xs_mask = in(reg) xs::MASK << FS_BIT,
            );
        }
        true
    }
}
