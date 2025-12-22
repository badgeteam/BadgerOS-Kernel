// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{arch::asm, ptr::NonNull};

use crate::scheduler::cpulocal::CpuLocal;

/// Architecture-specific CPU-local data.
#[repr(C)]
#[derive(Default)]
pub struct ArchCpuLocal {
    /// Stack pointer to use for interrupts; NULL to use current stack ptr.
    pub irq_stack: *mut (),
    /// Scratch space used by the trap and interrupt handlers.
    pub scratch: [usize; 3],
}

impl CpuLocal {
    /// Get the CPU-local pointer.
    #[inline(always)]
    pub fn get() -> Option<NonNull<CpuLocal>> {
        unsafe {
            let ptr: *mut Self;
            asm!("csrr {ptr}, sscratch", ptr=out(reg)ptr);
            NonNull::new(ptr)
        }
    }

    /// Set the CPU-local pointer.
    #[inline(always)]
    pub unsafe fn set(ptr: *mut Self) {
        unsafe { asm!("csrw sscratch, {ptr}",ptr=in(reg)ptr) };
    }
}
