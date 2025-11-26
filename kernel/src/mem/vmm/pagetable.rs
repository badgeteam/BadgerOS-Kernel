// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ops::Range,
    sync::atomic::{Atomic, Ordering},
};

use super::*;
use crate::{
    bindings::{error::EResult, raw::phys_page_free},
    cpu::mmu::{INVALID_PTE, PackedPTE},
    mem::pmm,
};

#[derive(Debug, Clone, Copy)]
/// Generic representation of a page table entry.
pub struct PTE {
    /// Physical page number that this PTE points to.
    pub ppn: PPN,
    /// Page protection flags, see [`super::flags`].
    pub flags: u32,
    /// At what level of the page table this PTE is stored.
    pub order: u8,
    /// Whether this PTE is valid.
    pub valid: bool,
    /// Whether this is a leaf PTE.
    pub leaf: bool,
}

impl PartialEq for PTE {
    fn eq(&self, other: &Self) -> bool {
        self.ppn == other.ppn
            && (self.flags & flags::RWX) == (other.flags & flags::RWX)
            && self.order == other.order
            && self.valid == other.valid
            && (self.leaf == other.leaf || !self.valid && !other.valid)
    }
}

impl PTE {
    /// The PTE that represents unmapped memory.
    pub const NULL: PTE = PTE {
        ppn: 0,
        flags: 0,
        order: 0,
        valid: false,
        leaf: false,
    };

    /// Whether this PTE represents unmapped memory (as some invalid PTEs may encode demand-mapped things).
    pub fn is_null(&self) -> bool {
        self.ppn == 0 && self.flags == 0 && !self.valid
    }
}

/// Version of a [`PTE`] that owns the page(s) in question.
#[derive(Debug)]
pub struct OwnedPTE(PTE);

impl PartialEq for OwnedPTE {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl OwnedPTE {
    /// The PTE that represents unmapped memory.
    pub const NULL: OwnedPTE = OwnedPTE(PTE::NULL);

    /// Whether this PTE owns RAM; it is safe to forget it if this returns false.
    pub fn owns_ram(&self) -> bool {
        self.0.valid && self.0.flags & flags::MODE != flags::MMIO
    }

    /// Create a copy from a [`PTE`] assumed to be sound, increasing the refcount if applicable.
    pub unsafe fn from_raw_ref(value: PTE) -> Self {
        if value.valid && value.flags & flags::MODE != flags::MMIO {
            unsafe { &*pmm::page_struct_base(value.ppn).0 }
                .refcount
                .fetch_add(1, Ordering::Relaxed);
        }
        Self(value)
    }

    /// Create a copy from a [`PTE`] assumed to be sound, without increasing refcount.
    pub unsafe fn from_raw_owned(value: PTE) -> Self {
        Self(value)
    }

    /// Create a new PTE for I/O memory.
    pub fn new_io(ppn: PPN, order: u8, flags: u32) -> Self {
        debug_assert!(
            order < unsafe { PAGING_LEVELS } as u8,
            "Attempt to create PTE that is too large"
        );
        debug_assert!(
            ppn >> order as u32 * BITS_PER_LEVEL << order as u32 * BITS_PER_LEVEL == ppn,
            "Attempt to create misaligned PTE"
        );
        debug_assert!(
            flags & flags::RX == flags::R,
            "Attempt to create I/O PTE with invalid protection flags"
        );
        debug_assert!(
            flags & flags::HHDM != 0
                || !pmm::page_range().contains(&ppn)
                || unsafe { (*pmm::page_struct(ppn)).usage() == pmm::PageUsage::Unusable },
            "Attempt to create I/O PTE that describes RAM"
        );
        Self(PTE {
            ppn,
            flags: (flags & !flags::MODE) | flags::MMIO,
            order,
            valid: true,
            leaf: true,
        })
    }

    /// Create a new PTE for RAM.
    pub fn new_ram(block: PhysPtr, offset: PPN, order: u8, flags: u32) -> Self {
        let ppn = block.into_raw_parts().0 + offset;
        debug_assert!(
            order < unsafe { PAGING_LEVELS } as u8,
            "Attempt to create PTE that is too large"
        );
        debug_assert!(
            ppn >> order as u32 * BITS_PER_LEVEL << order as u32 * BITS_PER_LEVEL == ppn,
            "Attempt to create misaligned PTE"
        );
        debug_assert!(
            flags & flags::R != 0,
            "Attempt to create RAM PTE with invalid protection flags"
        );
        debug_assert!(
            pmm::page_range().contains(&ppn)
                && unsafe { (*pmm::page_struct(ppn)).usage() != pmm::PageUsage::Unusable },
            "Attempt to create RAM PTE that describes I/O"
        );
        debug_assert!(
            flags & flags::MODE != flags::MMIO,
            "Attempt to create RAM PTE with mode MMIO"
        );
        Self(PTE {
            ppn,
            flags,
            order,
            valid: true,
            leaf: true,
        })
    }

    /// Physical page number that this PTE points to.
    pub fn ppn(&self) -> PPN {
        self.0.ppn
    }

    /// Page protection flags, see [`super::flags`].
    pub fn flags(&self) -> u32 {
        self.0.flags
    }

    /// At what level of the page table this PTE is stored.
    pub fn order(&self) -> u8 {
        self.0.order
    }

    /// Whether this PTE is valid.
    pub fn valid(&self) -> bool {
        self.0.valid
    }

    /// Whether this is a leaf PTE.
    pub fn leaf(&self) -> bool {
        self.0.leaf
    }

    /// Get the underlying page table entry.
    pub fn pte(&self) -> PTE {
        self.0
    }

    /// Convert into [`PTE`] without dropping the page(s)' refcount.
    pub fn into_raw(self) -> PTE {
        let tmp = self.0;
        core::mem::forget(self);
        tmp
    }

    /// Whether this PTE represents unmapped memory (as some invalid PTEs may encode demand-mapped things).
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }
}

impl Drop for OwnedPTE {
    fn drop(&mut self) {
        if !self.0.valid {
            return;
        }
        if self.0.leaf {
            if self.0.valid && self.0.flags & flags::MODE != flags::MMIO {
                unsafe { pmm::page_free(self.0.ppn) };
            }
        } else {
            unsafe { PageTable::drop_impl(self.0.ppn, self.0.order) };
        }
    }
}

impl Clone for OwnedPTE {
    fn clone(&self) -> Self {
        if self.owns_ram() {
            unsafe { &*pmm::page_struct_base(self.0.ppn).0 }
                .refcount
                .fetch_add(1, Ordering::Relaxed);
        }
        Self(self.0.clone())
    }
}

/// Abstracts direct manipulation and usage of page tables.
#[repr(C)]
pub struct PageTable {
    /// Root page number.
    root_ppn: PPN,
}

impl PageTable {
    /// Create a new page table.
    pub fn new() -> EResult<Self> {
        Ok(Self {
            root_ppn: alloc_pgtable_page()?,
        })
    }

    /// Get the root page number.
    pub fn root_ppn(&self) -> PPN {
        self.root_ppn
    }

    /// Preallocate memory to create a mapping.
    pub fn prealloc(&self, vpn: VPN, level: u8) -> EResult<()> {
        unsafe { self.map_raw_impl(vpn, None, level) }.map(|_| ())
    }

    /// Create a new mapping in the page table.
    /// Returns the old mapping, if it existed.
    pub unsafe fn map(&self, vpn: VPN, new_pte: OwnedPTE) -> EResult<OwnedPTE> {
        let tmp = new_pte.0;
        let res = unsafe { self.map_raw(vpn, new_pte)? };
        #[cfg(debug_assertions)]
        {
            let read = self.walk(vpn);
            debug_assert!(
                read == tmp,
                "Mapped {:#x?} doesn't match expected {:#x?}",
                read,
                tmp
            );
        }
        Ok(res)
    }

    /// Create a new mapping without testing for validity of said mapping.
    pub unsafe fn map_raw(&self, vpn: VPN, new_pte: OwnedPTE) -> EResult<OwnedPTE> {
        let level = new_pte.order();
        unsafe { self.map_raw_impl(vpn, Some(new_pte), level) }
    }

    unsafe fn map_raw_impl(
        &self,
        vpn: VPN,
        new_pte: Option<OwnedPTE>,
        level: u8,
    ) -> EResult<OwnedPTE> {
        let mut pgtable_ppn = self.root_ppn;
        let null_pte = new_pte.as_ref().map(|x| x.is_null()).unwrap_or(false);
        let global_flag = is_canon_kernel_page(vpn) as u32 * flags::G;
        let mut old_pte = None;

        // Descend the page table to the target level.
        for level in (level + 1..unsafe { PAGING_LEVELS as u8 }).rev() {
            let index = get_vpn_index(vpn, level);
            let pte = PTE::unpack(unsafe { read_pte(pgtable_ppn, index) }, level);

            pgtable_ppn = if !pte.valid {
                // Create a new level of page table.
                if null_pte {
                    // Unless the new PTE is null.
                    return Ok(OwnedPTE::NULL);
                }
                let ppn = alloc_pgtable_page()?;
                unsafe {
                    xchg_pte(
                        pgtable_ppn,
                        index,
                        PTE {
                            ppn,
                            flags: global_flag,
                            valid: true,
                            leaf: false,
                            order: level,
                        }
                        .pack(),
                    )
                };
                ppn
            } else if pte.leaf {
                // A superpage is split into smaller pages.
                let ppn = split_pgtable_leaf(pte, level - 1)?;
                old_pte = Some(OwnedPTE(PTE {
                    ppn: pte.ppn + get_vpn_index(vpn, level - 1),
                    flags: pte.flags,
                    valid: true,
                    leaf: true,
                    order: level - 1,
                }));
                unsafe {
                    xchg_pte(
                        pgtable_ppn,
                        index,
                        PTE {
                            ppn,
                            flags: global_flag,
                            valid: true,
                            leaf: false,
                            order: level,
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
            let index = get_vpn_index(vpn, new_pte.order());
            unsafe {
                let order = new_pte.order();
                let tmp = PTE::unpack(
                    xchg_pte(pgtable_ppn, index, new_pte.into_raw().pack()),
                    order,
                );
                if old_pte.is_none() {
                    old_pte = Some(OwnedPTE(tmp));
                }
            }
        }

        Ok(old_pte.unwrap_or(OwnedPTE::NULL))
    }

    /// Walk down the page table and read the target vaddr's PTE.
    #[inline(always)]
    pub fn walk(&self, vpn: VPN) -> PTE {
        self.walk_shallow(vpn, 0)
    }

    /// Walk down the page table and read the target vaddr's PTE.
    pub fn walk_shallow(&self, vpn: VPN, min_level: u32) -> PTE {
        debug_assert!(min_level < unsafe { PAGING_LEVELS });
        let mut pgtable_ppn = self.root_ppn;
        let mut pte;

        let _noirq = unsafe { IrqGuard::new() };

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

    /// Try to find the first non-NULL PTE starting at `min_vpn`.
    /// If `ignore_invalid_ptes` is true, it will return only the valid leaf PTEs.
    pub fn find_first(&self, mut min_vpn: VPN, ignore_invalid_ptes: bool) -> Option<(VPN, PTE)> {
        debug_assert!(is_canon_page(min_vpn));
        let canon_pages = canon_half_pages();

        let max_vpn = canon_pages * 2;
        min_vpn &= canon_pages - 1;

        while min_vpn < max_vpn {
            let pte = self.walk(min_vpn);
            if pte.valid || !ignore_invalid_ptes {
                let canon_vpn = if min_vpn >= canon_pages {
                    min_vpn.wrapping_sub(canon_pages)
                } else {
                    min_vpn
                };
                return Some((canon_vpn, pte));
            }
            min_vpn += 1 << BITS_PER_LEVEL * pte.order as u32;
        }

        None
    }

    /// Fill the higher half with empty pages.
    /// Used to construct the kernel page table.
    pub unsafe fn populate_higher_half(&mut self) -> EResult<()> {
        for i in PTE_PER_PAGE / 2..PTE_PER_PAGE {
            unsafe {
                xchg_pte(self.root_ppn, i, alloc_pgtable_page()?);
            }
        }
        Ok(())
    }

    /// Copy the higher-half mappings from what is assumed to be the kernel page table.
    pub unsafe fn copy_higher_half(&mut self, kernel_pt: &PageTable) {
        for i in PTE_PER_PAGE / 2..PTE_PER_PAGE {
            unsafe {
                xchg_pte(self.root_ppn, i, read_pte(kernel_pt.root_ppn, i));
            }
        }
    }

    /// Recursive implementation of the [`Drop`] trait.
    unsafe fn drop_impl(pgtable_ppn: PPN, level: u8) {
        unsafe {
            let range = if level == PAGING_LEVELS as u8 - 1 {
                // Won't clean up higher half; assuming here this is a user page table as the kernel one should never be dropped anyway.
                0..PTE_PER_PAGE / 2
            } else {
                0..PTE_PER_PAGE
            };
            for i in range {
                let pte = PTE::unpack(read_pte(pgtable_ppn, i), level);
                assert!(level > 0 || !pte.valid || pte.leaf);
                if pte.valid && !pte.leaf {
                    Self::drop_impl(pte.ppn, level - 1);
                } else {
                    drop(OwnedPTE::from_raw_owned(pte));
                }
            }
            phys_page_free(pgtable_ppn);
        }
    }
}

impl Drop for PageTable {
    fn drop(&mut self) {
        unsafe {
            Self::drop_impl(self.root_ppn, PAGING_LEVELS as u8 - 1);
        }
    }
}

/// How many bits of address space ID are available.
pub static mut ASID_BITS: u32 = 0;
/// Number of paging levels.
pub static mut PAGING_LEVELS: u32 = 0;
/// Number of PTEs per page.
pub const PTE_PER_PAGE: usize = PAGE_SIZE as usize / size_of::<PackedPTE>();

/// Get the index in the given page table level for the given virtual address.
fn get_vpn_index(vpn: VPN, level: u8) -> usize {
    (vpn >> (level as u32 * BITS_PER_LEVEL)) % (1usize << BITS_PER_LEVEL)
}

/// Read a PTE without any fencing or flushing.
unsafe fn read_pte(pgtable_ppn: PPN, index: usize) -> PackedPTE {
    let pte_vaddr =
        unsafe { HHDM_OFFSET } + pgtable_ppn * PAGE_SIZE as usize + index * size_of::<PackedPTE>();
    unsafe { (*(pte_vaddr as *mut Atomic<PackedPTE>)).load(Ordering::Acquire) }
}

/// Write a PTE without any fencing or flushing.
unsafe fn xchg_pte(pgtable_ppn: PPN, index: usize, pte: PackedPTE) -> PackedPTE {
    let pte_vaddr =
        unsafe { HHDM_OFFSET } + pgtable_ppn * PAGE_SIZE as usize + index * size_of::<PackedPTE>();
    unsafe { (*(pte_vaddr as *mut Atomic<PackedPTE>)).swap(pte, Ordering::AcqRel) }
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

    // Normally, the page table code itself doesn't manage page refcounts,
    // but this splitting increases the number of references.
    if orig.flags & flags::MODE != flags::MMIO {
        unsafe { &*pmm::page_struct_base(orig.ppn).0 }
            .refcount
            .fetch_add(PTE_PER_PAGE as u32 - 1, Ordering::Relaxed);
    }

    for i in 0..1usize << BITS_PER_LEVEL {
        unsafe {
            xchg_pte(
                ppn,
                i,
                PTE {
                    ppn: orig.ppn + (i << (new_level as u32 * BITS_PER_LEVEL)),
                    order: new_level,
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
pub fn higher_half_vaddr() -> usize {
    canon_half_size().wrapping_neg()
}

/// Get the start of the higher half.
pub fn higher_half_vpn() -> usize {
    higher_half_vaddr() / PAGE_SIZE as usize
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
