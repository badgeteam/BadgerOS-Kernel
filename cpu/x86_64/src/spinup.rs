// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::naked_asm;

use crate::{
    bindings::raw::limine_smp_info, boot::spinup::common_cpu_spinup,
    kernel::smp::limine_trampoline_2,
};

/// Run architecture-specific CPU spin-up code.
pub unsafe extern "C" fn arch_cpu_spinup() {
    unsafe {
        common_cpu_spinup();
    }
}

/// First stage trampoline for transferring control from Limine to BadgerOS.
#[unsafe(naked)]
pub unsafe extern "C" fn limine_trampoline_1(info: *mut limine_smp_info) {
    naked_asm!(
        "jmp {}",
        sym limine_trampoline_2
    );
}
