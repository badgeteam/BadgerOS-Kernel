// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ops::Range,
    sync::atomic::{Atomic, Ordering},
};

use super::*;
use crate::{
    bindings::{error::EResult, raw::phys_page_free},
    config::PAGE_SIZE,
    cpu::{
        self,
        mmu::{BITS_PER_LEVEL, INVALID_PTE, PackedPTE},
    },
    mem::{
        pmm::{self, PPN},
        vmm::VPN,
    },
};

pub static mut ASID_BITS: u32 = 0;
pub static mut PAGING_LEVELS: u32 = 0;

pub mod flags {
    pub use crate::cpu::mmu::flags::*;

    pub const RW: u32 = R | W;
    pub const RX: u32 = R | X;
    pub const RWX: u32 = R | W | X;
}

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

/// Physical mapping structure; informs the CPU of the virtual address map.
/// While it's safe to modify this structure in theory, actually providing it to the CPU is *unsafe*.
pub struct PhysMap {
    root: PPN,
}

impl PhysMap {
    pub fn new() -> EResult<Self> {
        todo!()
    }

    /// Get the physical page number of the root page table.
    /// While this structure is safe, actually providing it to the CPU is *unsafe*.
    pub const fn root(&self) -> PPN {
        self.root
    }

    /// Create or replace one page-sized mapping.
    pub fn map(&self) {}

    unsafe fn map_raw_impl(&self, vpn: VPN, new_pte: Option<PTE>, level: u8) -> EResult<()> {
        let mut pgtable_ppn = self.root;
        let null_pte = new_pte.as_ref().map(|x| x.is_null()).unwrap_or(false);
        let global_flag = is_canon_kernel_page(vpn) as u32 * flags::G;

        // Descend the page table to the target level.
        for level in (level + 1..unsafe { PAGING_LEVELS as u8 }).rev() {
            let index = get_vpn_index(vpn, level);
            let raw_pte = unsafe { read_pte(pgtable_ppn, index) };
            let pte = PTE::unpack(raw_pte, level);

            pgtable_ppn = if !pte.valid {
                // Create a new level of page table.
                if null_pte {
                    // Unless the new PTE is null.
                    return Ok(());
                }
                let ppn = alloc_pgtable_page()?;
                unsafe {
                    let res = cmpxchg_pte(
                        pgtable_ppn,
                        index,
                        raw_pte,
                        PTE {
                            ppn,
                            flags: global_flag,
                            valid: true,
                            leaf: false,
                            level,
                        }
                        .pack(),
                    );
                    if !res {
                        phys_page_free(ppn);
                        pgtable_ppn
                    } else {
                        ppn
                    }
                }
            } else if pte.leaf {
                // A superpage is split into smaller pages.
                let ppn = split_pgtable_leaf(pte, level - 1)?;
                unsafe {
                    xchg_pte(
                        pgtable_ppn,
                        index,
                        PTE {
                            ppn,
                            flags: global_flag,
                            valid: true,
                            leaf: false,
                            level,
                        }
                        .pack(),
                    )
                };
                ppn
            } else {
                pte.ppn
            };
        }

        // Write new PTE.
        if let Some(new_pte) = new_pte {
            let index = get_vpn_index(vpn, new_pte.level);
            unsafe {
                let order = new_pte.level;
                PTE::unpack(xchg_pte(pgtable_ppn, index, new_pte.pack()), order);
            }
        }

        Ok(())
    }

    /// Enable this physical map on this CPU.
    pub unsafe fn enable(&self) {
        unsafe {
            cpu::mmu::set_page_table(self.root, 0);
            cpu::mmu::vmem_fence(None, None);
        }
    }
}

impl Drop for PhysMap {
    fn drop(&mut self) {
        todo!()
    }
}

/// Get the index in the given page table level for the given virtual address.
#[inline(always)]
fn get_vpn_index(vpn: VPN, level: u8) -> usize {
    (vpn >> (level as u32 * BITS_PER_LEVEL)) % (1usize << BITS_PER_LEVEL)
}

/// Read a PTE without any fencing or flushing.
#[inline(always)]
unsafe fn read_pte(pgtable_ppn: PPN, index: usize) -> PackedPTE {
    let pte_vaddr =
        unsafe { HHDM_OFFSET } + pgtable_ppn * PAGE_SIZE as usize + index * size_of::<PackedPTE>();
    unsafe { (*(pte_vaddr as *mut Atomic<PackedPTE>)).load(Ordering::Acquire) }
}

/// Write a PTE without any fencing or flushing.
#[inline(always)]
unsafe fn xchg_pte(pgtable_ppn: PPN, index: usize, pte: PackedPTE) -> PackedPTE {
    let pte_vaddr =
        unsafe { HHDM_OFFSET } + pgtable_ppn * PAGE_SIZE as usize + index * size_of::<PackedPTE>();
    unsafe { (*(pte_vaddr as *mut Atomic<PackedPTE>)).swap(pte, Ordering::AcqRel) }
}

/// Compare-exchange a PTE.
#[inline(always)]
unsafe fn cmpxchg_pte(pgtable_ppn: PPN, index: usize, old: PackedPTE, new: PackedPTE) -> bool {
    let pte_vaddr =
        unsafe { HHDM_OFFSET } + pgtable_ppn * PAGE_SIZE as usize + index * size_of::<PackedPTE>();
    unsafe {
        (*(pte_vaddr as *mut Atomic<PackedPTE>))
            .compare_exchange_weak(old, new, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }
}

/// Try to allocate a new page table page.
fn alloc_pgtable_page() -> EResult<PPN> {
    let ppn = unsafe { pmm::page_alloc(0, pmm::PageUsage::PageTable) }?;
    for i in 0..1usize << BITS_PER_LEVEL {
        unsafe { xchg_pte(ppn, i, INVALID_PTE) };
    }
    Ok(ppn)
}

/// Determine the highest order of page that can be used for the start of a certain mapping.
#[inline(always)]
pub fn calc_superpage(vpn: VPN, ppn: PPN, size: VPN) -> u8 {
    ((vpn | ppn).trailing_zeros().min(size.ilog2()) / BITS_PER_LEVEL) as u8
}

/// Try to split a page table leaf node.
fn split_pgtable_leaf(orig: PTE, new_level: u8) -> EResult<PPN> {
    debug_assert!(orig.leaf && orig.valid);
    let ppn = unsafe { pmm::page_alloc(0, pmm::PageUsage::PageTable) }?;

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

/// Determine whether an address is canonical.
pub fn is_canon_page(addr: VPN) -> bool {
    // The upper (usually 12) bits of a VPN are ignored because a VPN is actually `usize::BITS - PAGE_SIZE.ilog2()` bits.
    let addr = (addr as isize) << PAGE_SIZE.ilog2() >> PAGE_SIZE.ilog2();
    let exp = usize::BITS - BITS_PER_LEVEL * unsafe { PAGING_LEVELS };
    let canon_page = (addr << exp) >> exp;
    canon_page == addr
}

/// Determine whether an address is a canonical kernel address.
pub fn is_canon_kernel_page(addr: VPN) -> bool {
    is_canon_page(addr) && (addr as isize) << PAGE_SIZE.ilog2() < 0
}

/// Determine whether an address is a canonical user address.
pub fn is_canon_user_page(addr: VPN) -> bool {
    is_canon_page(addr) && (addr as isize) >= 0
}

/// Determine whether an address is canonical.
pub fn is_canon_page_range(range: Range<VPN>) -> bool {
    is_canon_page(range.start) && (range.len() == 0 || is_canon_page(range.end - 1))
}

/// Determine whether an address is a canonical kernel address.
pub fn is_canon_kernel_page_range(range: Range<VPN>) -> bool {
    is_canon_kernel_page(range.start) && (range.len() == 0 || is_canon_kernel_page(range.end - 1))
}

/// Determine whether an address is a canonical user address.
pub fn is_canon_user_page_range(range: Range<VPN>) -> bool {
    is_canon_user_page(range.start) && (range.len() == 0 || is_canon_user_page(range.end - 1))
}

/// Get the size of a "half" of the canonical ranges.
pub fn canon_half_pages() -> usize {
    1 << (BITS_PER_LEVEL * unsafe { PAGING_LEVELS } - 1)
}

/// Get the size of a "half" of the canonical ranges.
pub fn canon_half_size() -> usize {
    (PAGE_SIZE as usize) << (BITS_PER_LEVEL * unsafe { PAGING_LEVELS } - 1)
}

/// Get the start of the higher half.
pub fn higher_half_page() -> VPN {
    VPN::MAX / PAGE_SIZE as VPN - canon_half_pages()
}

/// Get the start of the higher half.
pub fn higher_half_vaddr() -> usize {
    usize::MAX - canon_half_size()
}
