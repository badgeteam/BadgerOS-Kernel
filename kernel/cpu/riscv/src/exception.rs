// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{
    cpu::thread::{GpRegfile, SpRegfile},
    misc::panic::unhandled_trap,
};

/// RISC-V exception handler wrapper.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn riscv_exception_handler(regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    if sregs.scause < 0 {
        todo!("RISC-V interrupts");
    }

    // TODO: Certain traps can be handled.

    unhandled_trap(regs, sregs);
}
