use core::arch::{asm, naked_asm};

use crate::{
    arch::{
        kcore::smp::ArchSmp,
        riscv64::{Riscv, except::riscv_exception_vector},
    },
    bindings::{log::LogLevel, raw::limine_smp_info},
};

impl ArchSmp for Riscv {
    type CpuID = usize;

    fn cpu_spinup() {
        unsafe {
            asm!("csrw sstatus, 0");
            asm!("csrw stvec, {}", in(reg) riscv_exception_vector as *const ());
            asm!("csrw sie, {}", in(reg)(1 << 9)); // Supervisor external interrupt.
            logkf_unlocked!(LogLevel::Info, "STVEC init OK");
        }
    }

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
