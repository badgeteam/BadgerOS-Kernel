// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::{asm, naked_asm};

use crate::{
    bindings::raw::limine_smp_info, boot::spinup::common_cpu_spinup,
    kernel::smp::limine_trampoline_2,
};

unsafe extern "C" {
    /// RISC-V interrupt vector table.
    unsafe fn riscv_vector_table();
}

/// Run architecture-specific CPU spin-up code.
pub unsafe extern "C" fn arch_cpu_spinup() {
    unsafe {
        asm!("csrw sstatus, 0");
        asm!("csrw stvec, {}", in(reg) riscv_vector_table as *const () as usize);
        asm!("csrw sie, {}", in(reg)(1 << 9)); // Supervisor external interrupt.
        common_cpu_spinup();
    }
}

/// First stage trampoline for transferring control from Limine to BadgerOS.
#[unsafe(naked)]
pub unsafe extern "C" fn limine_trampoline_1(info: *mut limine_smp_info) {
    naked_asm!(
        ".option push",
        ".option norelax",
        "la gp, __global_pointer$",
        ".option pop",
        "j {}",
        sym limine_trampoline_2
    );
}
