// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::asm;

use crate::{cpu::CpuID, kernel::cpulocal::CpuLocal};

/// Architecture-specific CPU-local data.
#[repr(C)]
#[derive(Default)]
pub struct ArchCpuLocal {
    /// Stack pointer to use for interrupts; NULL to use current stack ptr.
    pub irq_stack: *mut (),
    /// Scratch space used by the trap and interrupt handlers.
    pub scratch: [usize; 3],
    /// What CPU ID this processor is.
    pub hartid: CpuID,
}

impl CpuLocal {
    /// Get the CPU-local pointer.
    #[inline(always)]
    pub fn get() -> *mut CpuLocal {
        unsafe {
            let ptr: *mut Self;
            asm!("csrr {ptr}, sscratch", ptr=out(reg)ptr);
            ptr
        }
    }

    /// Set the CPU-local pointer.
    #[inline(always)]
    pub unsafe fn set(ptr: *mut Self) {
        unsafe { asm!("csrw sscratch, {ptr}",ptr=in(reg)ptr) };
    }
}

impl ArchCpuLocal {
    /// Set the interrupt stack pointer.
    pub fn set_irq_stack(&mut self, sp: *mut ()) {
        self.irq_stack = sp;
    }
}
