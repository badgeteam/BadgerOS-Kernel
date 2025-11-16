// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ops::Range,
    sync::atomic::{Atomic, Ordering},
};

use crate::{
    badgelib::{irq::IrqGuard, rcu::RcuGuard},
    bindings::{error::EResult, log::print},
    config,
    cpu::mmu::{BITS_PER_LEVEL, INVALID_PTE, PackedPTE},
    mem::pmm::{PPN, page_alloc},
};

use super::*;

#[derive(Debug, Clone, Copy)]
/// Generic representation of a page table entry.
pub struct PTE {
    /// Physical page number that this PTE points to.
    pub ppn: PPN,
    /// Page protection flags, see [`super::flags`].
    pub flags: u32,
    /// At what level of the page table this PTE is stored.
    pub level: u8,
    /// Whether this PTE is valid.
    pub valid: bool,
    /// Whether this is a leaf PTE.
    pub leaf: bool,
}

impl PartialEq for PTE {
    fn eq(&self, other: &Self) -> bool {
        self.ppn == other.ppn
            && (self.flags & flags::RWX) == (other.flags & flags::RWX)
            && self.level == other.level
            && self.valid == other.valid
            && (self.leaf == other.leaf || !self.valid && !other.valid)
    }
}

impl PTE {
    /// The PTE that represents unmapped memory.
    pub const NULL: PTE = PTE {
        ppn: 0,
        flags: 0,
        level: 0,
        valid: false,
        leaf: false,
    };

    /// Whether this PTE represents unmapped memory (as some invalid PTEs may encode demand-mapped things).
    pub fn is_null(&self) -> bool {
        self.ppn == 0 && self.flags == 0 && !self.valid
    }
}

/// How many bits of address space ID are available.
pub static mut ASID_BITS: u32 = 0;
/// Number of paging levels.
pub static mut PAGING_LEVELS: u32 = 0;
/// Number of PTEs per page.
pub const PTE_PER_PAGE: usize = PAGE_SIZE as usize / size_of::<PackedPTE>();

/// Read a PTE without any fencing or flushing.
pub(super) unsafe fn read_pte(pgtable_ppn: PPN, index: usize) -> PackedPTE {
    let pte_vaddr = unsafe { HHDM_OFFSET }
        + pgtable_ppn * config::PAGE_SIZE as usize
        + index * size_of::<PackedPTE>();
    unsafe { (*(pte_vaddr as *mut Atomic<PackedPTE>)).load(Ordering::Acquire) }
}

/// Write a PTE without any fencing or flushing.
pub(super) unsafe fn xchg_pte(pgtable_ppn: PPN, index: usize, pte: PackedPTE) -> PackedPTE {
    let pte_vaddr = unsafe { HHDM_OFFSET }
        + pgtable_ppn * config::PAGE_SIZE as usize
        + index * size_of::<PackedPTE>();
    unsafe { (*(pte_vaddr as *mut Atomic<PackedPTE>)).swap(pte, Ordering::AcqRel) }
}

/// Get the index in the given page table level for the given virtual address.
fn get_vpn_index(vpn: VPN, level: u8) -> usize {
    (vpn >> (level as u32 * BITS_PER_LEVEL)) % (1usize << BITS_PER_LEVEL)
}

/// Try to allocate a new page table page.
pub(super) fn alloc_pgtable_page() -> EResult<PPN> {
    let ppn = unsafe { page_alloc(0, pmm::PageUsage::PageTable) }?;
    for i in 0..1usize << BITS_PER_LEVEL {
        unsafe { xchg_pte(ppn, i, INVALID_PTE) };
    }
    Ok(ppn)
}

/// Try to split a page table leaf node.
fn split_pgtable_leaf(orig: PTE, new_level: u8) -> EResult<PPN> {
    debug_assert!(orig.leaf && orig.valid);
    let ppn = unsafe { page_alloc(0, pmm::PageUsage::PageTable) }?;
    for i in 0..1usize << BITS_PER_LEVEL {
        unsafe {
            xchg_pte(
                ppn,
                i,
                PTE {
                    ppn: orig.ppn + (i << (new_level as u32 * BITS_PER_LEVEL)),
                    level: new_level,
                    ..orig
                }
                .pack(),
            )
        };
    }
    Ok(ppn)
}

/// Create a new mapping in the page table.
/// Returns whether the page table root was touched.
pub unsafe fn map(mut pgtable_ppn: PPN, vpn: VPN, new_pte: PTE) -> EResult<bool> {
    #[cfg(debug_assertions)]
    let orig_ppn = pgtable_ppn;
    debug_assert!(new_pte.leaf);
    let mut root_touched = false;
    // RCU guard not needed because only one thread may concurrently modify the page table.

    // Descend the page table to the target level.
    for level in (new_pte.level + 1..unsafe { PAGING_LEVELS as u8 }).rev() {
        let index = get_vpn_index(vpn, level as u8);
        let pte = PTE::unpack(unsafe { read_pte(pgtable_ppn, index) }, level);

        pgtable_ppn = if !pte.valid {
            // Create a new level of page table.
            if new_pte.is_null() {
                // Unless the new PTE is null.
                return Ok(false);
            }
            let ppn = alloc_pgtable_page()?;
            unsafe {
                xchg_pte(
                    pgtable_ppn,
                    index,
                    PTE {
                        ppn,
                        flags: new_pte.flags & flags::G,
                        valid: true,
                        leaf: false,
                        level,
                    }
                    .pack(),
                )
            };
            root_touched = true;
            ppn
        } else if pte.leaf {
            // Split up the leaf node.
            let ppn = split_pgtable_leaf(pte, level - 1)?;
            unsafe {
                xchg_pte(
                    pgtable_ppn,
                    index,
                    PTE {
                        ppn,
                        flags: new_pte.flags & flags::G,
                        valid: true,
                        leaf: false,
                        level,
                    }
                    .pack(),
                )
            };
            root_touched = true;
            ppn
        } else {
            pte.ppn
        };
    }

    // Write new PTE.
    let index = get_vpn_index(vpn, new_pte.level);
    unsafe {
        xchg_pte(pgtable_ppn, index, new_pte.pack());
    }

    #[cfg(debug_assertions)]
    {
        let tmp = unsafe { walk(orig_ppn, vpn) };
        debug_assert!(tmp == new_pte);
    };
    Ok(root_touched)
}

/// Walk down the page table and read the target vaddr's PTE.
pub unsafe fn walk(mut pgtable_ppn: PPN, vpn: VPN) -> PTE {
    let mut pte;
    let noirq = unsafe { IrqGuard::new() };
    let _rcu = unsafe { RcuGuard::new(&noirq) };

    // Descend the page until a leaf is found.
    for level in (0..unsafe { PAGING_LEVELS }).rev() {
        let index = get_vpn_index(vpn, level as u8);
        pte = PTE::unpack(unsafe { read_pte(pgtable_ppn, index) }, level as u8);

        if !pte.valid && level > 0 {
            return pte;
        } else if pte.valid && !pte.leaf {
            pgtable_ppn = pte.ppn;
        } else {
            return pte;
        }
    }

    unreachable!("Valid non-leaf PTE at level 0");
}

unsafe fn dump_impl(pgtable_ppn: PPN, level: u8, min_level: u8, indent: u8, vpn: VPN) {
    for index in 0..(1usize << BITS_PER_LEVEL) {
        let pte = PTE::unpack(unsafe { read_pte(pgtable_ppn, index) }, level);
        if pte.is_null() {
            continue;
        }
        use flags::*;
        for _ in 0..indent {
            print("    ");
        }
        printf!(
            "[{:3}] 0x{:x} {}{}{}{}{}{}{} {} {}",
            index,
            pte.ppn,
            if pte.flags & R != 0 { 'R' } else { '-' },
            if pte.flags & W != 0 { 'W' } else { '-' },
            if pte.flags & X != 0 { 'X' } else { '-' },
            if pte.flags & U != 0 { 'U' } else { '-' },
            if pte.flags & G != 0 { 'G' } else { '-' },
            if pte.flags & A != 0 { 'A' } else { '-' },
            if pte.flags & D != 0 { 'D' } else { '-' },
            if pte.flags & COW != 0 { "COW" } else { "---" },
            if pte.flags & IO != 0 {
                "IO"
            } else if pte.flags & NC != 0 {
                "NC"
            } else {
                "--"
            }
        );
        let mut vpn = vpn + (index << (BITS_PER_LEVEL * level as u32));
        if vpn >= canon_half_size() / PAGE_SIZE as usize {
            vpn += higher_half_vaddr() / PAGE_SIZE as usize;
        }
        if pte.leaf {
            printf!(" (leaf 0x{:x})\n", vpn);
        } else if level > min_level {
            print("\n");
            unsafe { dump_impl(pte.ppn, level - 1, min_level, indent + 1, vpn) };
        } else {
            print("\n");
        }
    }
}

/// Debug-dump a page tbale.
pub unsafe fn dump(pgtable_ppn: PPN, min_level: u8) {
    printf!("0x{:x} (page table root)\n", pgtable_ppn);
    unsafe { dump_impl(pgtable_ppn, PAGING_LEVELS as u8 - 1, min_level, 1, 0) };
}

/// Determine whether an address is canonical.
pub fn is_canon_addr(addr: usize) -> bool {
    let addr = addr as isize;
    let exp = usize::BITS - PAGE_SIZE.ilog2() - BITS_PER_LEVEL * unsafe { PAGING_LEVELS };
    let canon_addr = (addr << exp) >> exp;
    canon_addr == addr
}

/// Determine whether an address is a canonical kernel address.
pub fn is_canon_kernel_addr(addr: usize) -> bool {
    is_canon_addr(addr) && (addr as isize) < 0
}

/// Determine whether an address is a canonical user address.
pub fn is_canon_user_addr(addr: usize) -> bool {
    is_canon_addr(addr) && (addr as isize) >= 0
}

/// Determine whether an address is canonical.
pub fn is_canon_range(range: Range<usize>) -> bool {
    is_canon_addr(range.start) && (range.len() == 0 || is_canon_addr(range.end - 1))
}

/// Determine whether an address is a canonical kernel address.
pub fn is_canon_kernel_range(range: Range<usize>) -> bool {
    is_canon_kernel_addr(range.start) && (range.len() == 0 || is_canon_kernel_addr(range.end - 1))
}

/// Determine whether an address is a canonical user address.
pub fn is_canon_user_range(range: Range<usize>) -> bool {
    is_canon_user_addr(range.start) && (range.len() == 0 || is_canon_user_addr(range.end - 1))
}

/// Get the size of a "half" of the canonical ranges.
pub fn canon_half_size() -> usize {
    (PAGE_SIZE as usize) << (BITS_PER_LEVEL * unsafe { PAGING_LEVELS } - 1)
}

/// Get the start of the higher half.
pub fn higher_half_vaddr() -> usize {
    canon_half_size().wrapping_neg()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mmu_is_canon_addr(addr: usize) -> bool {
    is_canon_addr(addr)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mmu_is_canon_kernel_addr(addr: usize) -> bool {
    is_canon_kernel_addr(addr)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mmu_is_canon_user_addr(addr: usize) -> bool {
    is_canon_user_addr(addr)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mmu_is_canon_range(start: usize, len: usize) -> bool {
    is_canon_range(start..start + len)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mmu_is_canon_kernel_range(start: usize, len: usize) -> bool {
    is_canon_kernel_range(start..start + len)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mmu_is_canon_user_range(start: usize, len: usize) -> bool {
    is_canon_user_range(start..start + len)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mmu_canon_half_size() -> usize {
    canon_half_size()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mmu_higher_half_vaddr() -> usize {
    higher_half_vaddr()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mmu_paging_levels() -> i32 {
    unsafe { PAGING_LEVELS as i32 }
}
