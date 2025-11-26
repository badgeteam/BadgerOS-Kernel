// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::asm;

use crate::mem::{
    pmm::PPN,
    vmm::pagetable::{ASID_BITS, PAGING_LEVELS, PTE},
};

pub mod flags {
    /// PTE valid bit.
    pub const P: u32 = 0b0_0000_0001;
    /// Map memory as writeable (reads must also be allowed).
    pub const W: u32 = 0b0_0000_0010;
    /// Map memory as user-accessible.
    pub const U: u32 = 0b0_0000_0100;
    /// Map memory as I/O (uncached, no write coalescing).
    pub const IO: u32 = 0b0_0000_1000;
    /// Map memory as uncached write coalescing.
    pub const NC: u32 = 0b0_0001_0000;
    /// Page was accessed since this flag was last cleared.
    pub const A: u32 = 0b0_0010_0000;
    /// Page was written since this flag was last cleared.
    pub const D: u32 = 0b0_0100_0000;
    /// Map memory as global (exists in all page ASIDs).
    pub const G: u32 = 0b1_0000_0000;
    /// This is a hugepage leaf.
    pub const PS: u32 = 0b0_1000_0000;

    /// Mark page as copy-on-write (W must be disabled).
    pub const COW: u32 = 0b0010_0000_0000;
    /// Mark page as shared (will not be turned into CoW on fork).
    pub const SHM: u32 = 0b0100_0000_0000;
    /// Mark page as memory-mapped I/O (anything except normal RAM; informational in case hardare doesn't support this flag).
    pub const MMIO: u32 = 0b0110_0000_0000;
    /// What kind of memory is mapped at this page.
    pub const MODE: u32 = 0b0110_0000_0000;

    /// Dummy readable flag.
    pub const R: u32 = 1 << 16;
    /// Mark memory as executable (removes the XD flag).
    pub const X: u32 = 1 << 17;
}

/// Data type that can store a packed page table entry.
pub type PackedPTE = usize;

/// An invalid PTE with no special data in it.
pub const INVALID_PTE: PackedPTE = 0;

impl PTE {
    /// Pack this PTE.
    pub fn pack(self) -> PackedPTE {
        debug_assert!(self.flags & flags::R != 0);
        let mut flags = self.flags as usize & 0x1ff;
        if self.flags & flags::X != 0 {
            flags |= 1 << 63;
        }
        if self.leaf && self.order > 0 {
            flags |= flags::PS as usize;
        }
        let ppn = self.ppn << 12;
        flags | ppn
    }

    /// Unpack this PTE.
    pub fn unpack(packed: PackedPTE, order: u8) -> PTE {
        let mut ppn = packed & 0x000f_ffff_ffff_f000;
        let leaf;
        if order == 0 {
            leaf = true;
        } else {
            leaf = packed & flags::PS as usize != 0;
            ppn &= !(1 << 12);
        }
        let mut flags = (packed as u32 & 0x1ff) | flags::R;
        if packed & (1 << 63) == 0 {
            flags |= flags::X;
        }
        PTE {
            ppn,
            flags,
            order,
            valid: packed & flags::P as usize != 0,
            leaf,
        }
    }
}

/// Maximum possible value of ASID.
pub const ASID_MAX: u32 = 0xffff;
/// Number of virtual address bits per page table level.
pub const BITS_PER_LEVEL: u32 = 9;
/// Heuristic for maximum number of pages to individually invalidate.
pub const INVAL_PAGE_THRESHOLD: usize = 16;

/// Perform early MMU initialization using the existing page tables (which were created by the bootloader).
pub unsafe fn early_init() {
    unsafe {
        let cr4: usize;
        asm!("mov {cr4}, cr4", cr4 = out(reg) cr4);
        if cr4 & (1 << 12) != 0 {
            PAGING_LEVELS = 5;
        } else {
            PAGING_LEVELS = 4;
        }
    }
}

/// Initialize and detect capabilities of the MMU, given the constructed page table.
pub unsafe fn init(root_ppn: PPN) {
    unsafe {
        ASID_BITS = 0;

        set_page_table(root_ppn, 0);
    }
}

#[inline(always)]
/// Switch page table and address space ID.
pub unsafe fn set_page_table(root_ppn: PPN, asid: usize) {
    let cr3 = asid | (root_ppn << 12);
    unsafe {
        asm!("mov cr3, {cr3}", cr3 = in(reg) cr3);
    }
}

#[inline(always)]
/// Perform a fence of virtual memory.
pub fn vmem_fence(vaddr: Option<usize>, _asid: Option<usize>) {
    unsafe {
        if let Some(vaddr) = vaddr {
            asm!("invlpg [{vaddr}]", vaddr = in(reg) vaddr);
        } else {
            asm!("mov rax, cr3", "mov cr3, rax", out("rax") _);
        }
    }
}
