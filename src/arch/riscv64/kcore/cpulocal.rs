use core::arch::asm;

use crate::{
    arch::{
        kcore::cpulocal::{ArchCpuLocal, ArchCpuLocalData},
        riscv64::Riscv,
    },
    kcore::cpulocal::CpuLocal,
};

// RISC-V keeps CPU-local pointer in `sscratch`, and uses `tp` for convenience while in kernel mode.
impl ArchCpuLocal for Riscv {
    type CpuLocalData = RiscvCpuLocalData;

    #[inline(always)]
    fn get_cpulocal() -> *mut CpuLocal {
        let ptr;
        unsafe { asm!("mv {}, tp", out(reg)ptr, options(nomem, nostack)) };
        ptr
    }

    #[inline(always)]
    unsafe fn set_cpulocal(next: *mut CpuLocal) {
        unsafe { asm!("mv tp, {}", in(reg)next) };
        unsafe { asm!("csrw sscratch, {}", in(reg)next) };
    }
}

#[repr(C)]
#[derive(Default)]
pub struct RiscvCpuLocalData {
    pub scratch: [usize; 3],
    pub irq_stack: *mut (),
}

impl ArchCpuLocalData for RiscvCpuLocalData {
    fn set_irq_stack(&mut self, sp: *mut ()) {
        self.irq_stack = sp;
    }
}
