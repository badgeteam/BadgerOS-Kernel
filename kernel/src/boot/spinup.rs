// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;

use crate::{cpu::cpulocal::ArchCpuLocal, scheduler::cpulocal::CpuLocal};

/// Common CPU spin-up routine.
/// Should only be called by [`crate::cpu::spinup::arch_cpu_spinup`].
pub unsafe fn common_cpu_spinup() {
    let cpulocal = Box::into_raw(Box::new(CpuLocal {
        arch: ArchCpuLocal::default(),
        thread: None,
        sched: None,
    }));
    unsafe { CpuLocal::set(cpulocal) };
}
