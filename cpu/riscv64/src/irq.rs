use core::arch::asm;

use crate::bindings::raw::CSR_STATUS_IE_BIT;

/// Check whether interrupts are enabled.
#[inline(always)]
pub fn is_enabled() -> bool {
    let mut mask: usize;
    unsafe {
        asm!("csrr {tmp}, sstatus", tmp = out(reg) mask);
    }
    mask & (1 << CSR_STATUS_IE_BIT) != 0
}

/// Disable interrupts if some condition holds.
#[inline(always)]
pub unsafe fn disable_if(cond: bool) -> bool {
    let mut mask: usize = (cond as usize) << CSR_STATUS_IE_BIT;
    unsafe {
        asm!("csrrc {tmp}, sstatus, {tmp}", tmp = inout(reg) mask);
    }
    mask & (1 << CSR_STATUS_IE_BIT) != 0
}

/// Enable interrupts if some condition holds.
#[inline(always)]
pub unsafe fn enable_if(cond: bool) {
    let mask: usize = (cond as usize) << CSR_STATUS_IE_BIT;
    unsafe {
        asm!("csrs sstatus, {tmp}", tmp = in(reg) mask);
    }
}

/// Set or clear a bit in the supervisor interrupt-enable CSR (`sie`).
/// `bit` is the interrupt cause number (e.g. 9 for supervisor external, 5 for timer).
///
/// # Safety
/// Enabling an interrupt source whose handler is not yet installed may cause an
/// unhandled-trap panic when it fires.
#[inline(always)]
pub unsafe fn set_sie_bit(bit: usize, enable: bool) {
    let mask: usize = 1 << bit;
    unsafe {
        if enable {
            asm!("csrs sie, {mask}", mask = in(reg) mask);
        } else {
            asm!("csrc sie, {mask}", mask = in(reg) mask);
        }
    }
}

/// Disable interrupts.
#[inline(always)]
pub unsafe fn disable() -> bool {
    unsafe { disable_if(true) }
}

/// Enable interrupts.
#[inline(always)]
pub unsafe fn enable() {
    unsafe { enable_if(true) }
}
