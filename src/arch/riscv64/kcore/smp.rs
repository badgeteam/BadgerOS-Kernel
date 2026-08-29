use core::arch::naked_asm;

use crate::{
    arch::{kcore::smp::ArchSmp, riscv64::Riscv},
    bindings::raw::limine_smp_info,
};

impl ArchSmp for Riscv {
    type CpuID = usize;

    #[unsafe(naked)]
    unsafe extern "C" fn limine_trampoline_1(info: *mut limine_smp_info) {
        naked_asm!(
            ".option push",
            ".option norelax",
            "la gp, __global_pointer$",
            ".option pop",
            "j {}",
            sym crate::kcore::smp::limine_trampoline_2
        );
    }
}
