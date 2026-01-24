// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::asm;

use crate::kernel::cpulocal::CpuLocal;

/// Architecture-specific CPU-local data.
#[repr(C)]
#[derive(Default)]
pub struct ArchCpuLocal {
    /// Pointer to the CPU-local data as a whole.
    self_ptr: *mut CpuLocal,
}

impl CpuLocal {
    /// Get the CPU-local pointer.
    #[inline(always)]
    pub fn get() -> *mut CpuLocal {
        unsafe {
            let ptr: *mut Self;
            asm!("mov {ptr}, [gs:0]", ptr=out(reg)ptr);
            ptr
        }
    }

    /// Set the CPU-local pointer.
    #[inline(always)]
    pub unsafe fn set(ptr: *mut Self) {
        unsafe {
            (*ptr).arch.self_ptr = ptr;
        };
    }
}

impl ArchCpuLocal {
    /// Set the interrupt stack pointer.
    pub fn set_irq_stack(&mut self, sp: *mut ()) {}
}
