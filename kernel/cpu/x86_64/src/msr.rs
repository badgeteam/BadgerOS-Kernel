// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::asm;

pub mod fsbase {
    /// MSR: Base address of `fs` segment.
    pub const ADDR: u32 = 0xc0000100;
}

pub mod gsbase {
    /// MSR: Base address of `gs` segment.
    /// Swapped with KGSBASE using the `swapgs` instruction.
    pub const ADDR: u32 = 0xc0000101;
}

pub mod kgsbase {
    /// MSR: Temporary value for kernel `gs` segment.
    /// Swapped with GSBASE using the `swapgs` instruction.
    pub const ADDR: u32 = 0xc0000102;
}

pub mod efer {
    /// MSR: Extended Feature Enable Register..
    pub const ADDR: u32 = 0xc0000080;

    /// EFER: System call extensions.
    pub const SCE_MASK: u32 = 1 << 0;
    /// EFER: Long mode enable.
    pub const LME_MASK: u32 = 1 << 8;
    /// EFER: Long mode active.
    pub const LMA_MASK: u32 = 1 << 9;
    /// EFER: No-execute enable.
    pub const NXE_MASK: u32 = 1 << 10;
    /// EFER: Secure virtual machine enable.
    pub const SVME_MASK: u32 = 1 << 11;
    /// EFER: Fast FXSAVE/FXSTOR.
    pub const FFXSR_MASK: u32 = 1 << 12;
    /// EFER: Translation cache extension.
    pub const TCE_MASK: u32 = 1 << 13;
}

pub mod star {
    /// MSR: CS/SS for user/kernel.
    pub const ADDR: u32 = 0xc0000081;
}

pub mod lstar {
    /// MSR: Entry point for system calls.
    pub const ADDR: u32 = 0xc0000082;
}

pub mod fmask {
    /// MSR: Flags to clear when entering kernel.
    pub const ADDR: u32 = 0xc0000084;
}

/// Read an MSR.
pub unsafe fn read(addr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!("rdmsr", in("ecx")addr, out("eax")lo, out("edx")hi, options(nostack, readonly, preserves_flags));
    }
    ((hi as u64) << 32) + lo as u64
}

/// Write an MSR.
pub unsafe fn write(addr: u32, value: u64) {
    unsafe {
        asm!("wrmsr", in("ecx")addr, in("edx")(value >> 32) as u32, in("eax")value as u32, options(nostack, preserves_flags));
    }
}
