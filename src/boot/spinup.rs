// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

/// Common CPU spin-up routine.
/// Should only be called by [`crate::cpu::spinup::arch_cpu_spinup`].
pub unsafe fn common_cpu_spinup() {
    // TODO: Set up CPU-local interrupt stack.
}
