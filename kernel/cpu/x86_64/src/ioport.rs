// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::asm;

pub unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx")port, in("al")value, options(preserves_flags));
    }
}

pub unsafe fn outw(port: u16, value: u16) {
    unsafe {
        asm!("out dx, ax", in("dx")port, in("ax")value, options(preserves_flags));
    }
}

pub unsafe fn outd(port: u16, value: u32) {
    unsafe {
        asm!("out dx, eax", in("dx")port, in("eax")value, options(preserves_flags));
    }
}

pub unsafe fn inb(port: u16) -> u8 {
    let res;
    unsafe {
        asm!("in al, dx", in("dx")port, out("al")res);
    }
    res
}

pub unsafe fn inw(port: u16) -> u16 {
    let res;
    unsafe {
        asm!("in ax, dx", in("dx")port, out("ax")res);
    }
    res
}

pub unsafe fn ind(port: u16) -> u32 {
    let res;
    unsafe {
        asm!("in eax, dx", in("dx")port, out("eax")res);
    }
    res
}
