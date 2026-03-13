// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use super::raw::isr_ctx_t;
use core::arch::asm;

/// Get the current ISR context.
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
#[inline(always)]
pub fn isr_ctx_get() -> *mut isr_ctx_t {
    let tmp: *mut isr_ctx_t;
    unsafe { asm!("csrr {t}, sscratch", t = out(reg) tmp) };
    tmp
}

/// Get the current ISR context.
#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub fn isr_ctx_get() -> *mut isr_ctx_t {
    todo!();
}
