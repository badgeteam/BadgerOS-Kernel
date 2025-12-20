// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::asm;

use crate::bindings::log::write_unlocked;

/// Get the frame pointer register.
pub fn get_frame_ptr() -> *const () {
    let tmp;
    unsafe { asm!("mv {}, s0", out(reg)tmp) };
    tmp
}

/// Start a backtrace using a given frame pointer.
pub fn backtrace(fp: *const ()) {
    let mut fp = fp as *const usize;
    // Prev FP offset: -2 words
    // Prev RA offset: -1 word
    write_unlocked("**** BEGIN BACKRTACE ****\n");
    loop {
        let ra: usize;
        if (fp as isize) >= 0 {
            break;
        }
        ra = unsafe { *fp.wrapping_sub(1) }; // TODO: Safe copy.
        #[cfg(target_arch = "riscv32")]
        printf_unlocked!("0x{:08x}\n", ra);
        #[cfg(target_arch = "riscv64")]
        printf_unlocked!("0x{:016x}\n", ra);
        if (fp as isize) >= 0 {
            break;
        }
        fp = unsafe { *fp.wrapping_sub(2) } as *const usize; // TODO: Safe copy.
    }
    write_unlocked("**** END BACKRTACE ****\n");
}
