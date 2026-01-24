use core::arch::asm;

/// Check whether interrupts are enabled.
#[inline(always)]
pub fn is_enabled() -> bool {
    let tmp: usize;
    // Pure and readonly options mean this will be eliminated if the result is unused.
    unsafe { asm!("pushf; pop {tmp}", tmp = out(reg) tmp, options(pure, readonly)) };
    tmp & (1 << 9) != 0
}

/// Disable interrupts if some condition holds.
#[inline(always)]
pub unsafe fn disable_if(cond: bool) -> bool {
    let enabled = unsafe { is_enabled() };
    if cond {
        unsafe { asm!("cli") };
    }
    enabled
}

/// Enable interrupts if some condition holds.
#[inline(always)]
pub unsafe fn enable_if(cond: bool) {
    if cond {
        unsafe { asm!("sti") };
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
