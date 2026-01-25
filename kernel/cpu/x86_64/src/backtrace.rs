// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::asm;

use crate::bindings::log::write_unlocked;

/// Get the frame pointer register.
pub fn get_frame_ptr() -> *const () {
    let tmp;
    unsafe { asm!("mov {}, rbp", out(reg)tmp, options(nomem)) };
    tmp
}

/// Start a backtrace using a given frame pointer.
pub fn backtrace(fp: *const ()) {
    let mut fp = fp as *const usize;
    // Prev RBP offset: 0 words
    // Prev RIP offset: 1 word
    write_unlocked("**** BEGIN BACKRTACE ****\n");
    loop {
        let ra: usize;
        if (fp as isize) >= 0 {
            break;
        }
        ra = unsafe { *fp.wrapping_add(1) }; // TODO: Safe copy.
        printf_unlocked!("0x{:016x}\n", ra);
        if (fp as isize) >= 0 {
            break;
        }
        fp = unsafe { *fp } as *const usize; // TODO: Safe copy.
    }
    write_unlocked("**** END BACKRTACE ****\n");
}
