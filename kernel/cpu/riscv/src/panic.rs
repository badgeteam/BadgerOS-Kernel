// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::asm;

use crate::cpu::irq;

/// Immediately shut down or spin the current CPU.
pub fn panic_cpu_shutdown() -> ! {
    unsafe {
        irq::disable();
        loop {
            asm!("pause");
        }
    }
}
