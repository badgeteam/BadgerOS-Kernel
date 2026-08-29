use core::arch::asm;

use crate::{
    arch::{kcore::cpulocal::ArchCpuLocal, riscv64::Riscv},
    kcore::cpulocal::CpuLocal,
};

// RISC-V keeps CPU-local pointer in `sscratch`, and uses `tp` for convenience while in kernel mode.
impl ArchCpuLocal for Riscv {
    type ExtraData = [usize; 3];

    #[inline(always)]
    fn get() -> *mut CpuLocal {
        let ptr;
        unsafe { asm!("mv {}, tp", out(reg)ptr, options(nomem, nostack)) };
        ptr
    }

    #[inline(always)]
    unsafe fn set(next: *mut CpuLocal) {
        unsafe { asm!("mv tp, {}", in(reg)next) };
        unsafe { asm!("csrw sscratch, {}", in(reg)next) };
    }
}
