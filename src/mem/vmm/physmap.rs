// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    fmt::Debug,
    ops::Range,
    sync::atomic::{Atomic, Ordering},
};

use super::*;
use crate::{
    badgelib::irq::IrqGuard,
    bindings::{error::EResult, raw::phys_page_free},
    config::PAGE_SIZE,
    cpu::{
        self,
        mmu::{BITS_PER_LEVEL, INVALID_PTE, PackedPTE},
    },
    mem::pmm,
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
    pub ppn: usize,
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
    root: PAddrr,
}

impl PhysMap {
    pub(super) fn new() -> EResult<Self> {
        Ok(Self {
            root: alloc_pgtable_page()?,
        })
    }

    /// Pre-populate the higher half of the root page table.
    /// Used in the creation of the kernel page table.
    pub(super) unsafe fn populate_higher_half(&self) -> EResult<()> {
        for i in 1 << (BITS_PER_LEVEL - 1)..1 << BITS_PER_LEVEL {
            let paddr = alloc_pgtable_page()?;
            unsafe {
                xchg_pte(
                    self.root,
                    i,
                    PTE {
                        ppn: paddr / PAGE_SIZE as usize,
                        flags: mmu::flags::G,
                        level: PAGING_LEVELS as u8 - 2,
                        valid: true,
                        leaf: false,
                    }
                    .pack(),
                );
            }
        }

        Ok(())
    }

    /// Broadcast the higher half from one pmap to another.
    /// Used in the creation of user page tables.
    pub(super) unsafe fn broadcast_higher_half(from: &Self, to: &Self) {
        for i in 1 << (BITS_PER_LEVEL - 1)..1 << BITS_PER_LEVEL {
            unsafe {
                let raw = read_pte(from.root, i);
                xchg_pte(to.root, i, raw);
            }
        }
    }

    /// Get the physical page number of the root page table.
    /// While this structure is safe, actually providing it to the CPU is *unsafe*.
    pub const fn root(&self) -> PAddrr {
        self.root
    }

    /// Create or replace one page-sized mapping.
    pub unsafe fn map(&self, vaddr: usize, paddr: PAddrr, flags: u32) -> EResult<()> {
        unsafe {
            debug_assert!(paddr % PAGE_SIZE as usize == 0);
            self.map_raw_impl(
                vaddr,
                Some(PTE {
                    ppn: paddr / PAGE_SIZE as usize,
                    flags,
                    level: 0,
                    valid: true,
                    leaf: true,
                }),
                0,
            )
        }
    }

    /// Delete one page-sized mapping.
    pub unsafe fn unmap(&self, vaddr: usize) {
        unsafe {
            self.map_raw_impl(vaddr, None, 0)
                .expect("PhysMap::unmap failed")
        }
    }

    unsafe fn map_raw_impl(&self, vaddr: usize, new_pte: Option<PTE>, level: u8) -> EResult<()> {
        debug_assert!(vaddr % PAGE_SIZE as usize == 0);
        let mut pgtable_paddr = self.root;
        let null_pte = new_pte.as_ref().map(|x| x.is_null()).unwrap_or(false);
        let global_flag = is_canon_kernel_addr(vaddr) as u32 * flags::G;

        // Descend the page table to the target level.
        for level in (level + 1..unsafe { PAGING_LEVELS as u8 }).rev() {
            let index = get_vpn_index(vaddr, level);
            let raw_pte = unsafe { read_pte(pgtable_paddr, index) };
            let pte = PTE::unpack(raw_pte, level);

            pgtable_paddr = if !pte.valid {
                // Create a new level of page table.
                if null_pte {
                    // Unless the new PTE is null.
                    return Ok(());
                }
                let paddr = alloc_pgtable_page()?;
                unsafe {
                    let res = cmpxchg_pte(
                        pgtable_paddr,
                        index,
                        raw_pte,
                        PTE {
                            ppn: paddr / PAGE_SIZE as usize,
                            flags: global_flag,
                            valid: true,
                            leaf: false,
                            level,
                        }
                        .pack(),
                    );
                    if !res {
                        phys_page_free(paddr);
                        pgtable_paddr
                    } else {
                        paddr
                    }
                }
            } else if pte.leaf {
                // A superpage is split into smaller pages.
                let paddr = split_pgtable_leaf(pte, level - 1)?;
                unsafe {
                    // TODO: Currently unreachable, but this is incorrect.
                    xchg_pte(
                        pgtable_paddr,
                        index,
                        PTE {
                            ppn: paddr / PAGE_SIZE as usize,
                            flags: global_flag,
                            valid: true,
                            leaf: false,
                            level,
                        }
                        .pack(),
                    )
                };
                paddr
            } else {
                pte.ppn * PAGE_SIZE as usize
            };
        }

        // Write new PTE.
        if let Some(new_pte) = new_pte {
            let index = get_vpn_index(vaddr, new_pte.level);
            unsafe {
                let order = new_pte.level;
                PTE::unpack(xchg_pte(pgtable_paddr, index, new_pte.pack()), order);
            }
        }

        Ok(())
    }

    /// Delete multiple page-sized mappings.
    pub unsafe fn unmap_multiple(&self, mut vaddr: usize, mut size: usize) {
        debug_assert!(size % PAGE_SIZE as usize == 0);
        unsafe {
            while size > 0 {
                self.unmap(vaddr);
                vaddr += PAGE_SIZE as usize;
                size -= PAGE_SIZE as usize
            }
        }
    }

    /// Change protection flags for all mappings within the given range.
    /// Will only decrease access permission, never increase it.
    pub unsafe fn protect(&self, mut vaddr: usize, mut size: usize, max_prot: u32) {
        debug_assert!(size % PAGE_SIZE as usize == 0);
        unsafe {
            while size > 0 {
                let mut pte = self.walk(vaddr);
                let newfl = pte.flags & (max_prot | !flags::RWX);
                if newfl != pte.flags {
                    pte.flags = newfl;
                    self.map_raw_impl(vaddr, Some(pte), pte.level)
                        .expect("PhysMap::protect failed");
                }
                vaddr += PAGE_SIZE as usize;
                size -= PAGE_SIZE as usize
            }
        }
    }

    /// Walk down the page table and read the target vaddr's PTE.
    #[inline(always)]
    pub fn walk(&self, vaddr: usize) -> PTE {
        self.walk_shallow(vaddr, 0)
    }

    /// Walk down the page table and read the target vaddr's PTE.
    pub fn walk_shallow(&self, vaddr: usize, min_level: u32) -> PTE {
        debug_assert!(min_level < unsafe { PAGING_LEVELS });
        let mut pgtable_paddr = self.root;
        let mut pte;

        let _noirq = IrqGuard::new();

        // Descend the page until a leaf is found.
        for level in (0..unsafe { PAGING_LEVELS }).rev() {
            let index = get_vpn_index(vaddr, level as u8);
            pte = PTE::unpack(unsafe { read_pte(pgtable_paddr, index) }, level as u8);

            if level == min_level || !pte.valid && level > 0 {
                return pte;
            } else if pte.valid && !pte.leaf {
                debug_assert!(pte.ppn != 0);
                pgtable_paddr = pte.ppn * PAGE_SIZE as usize;
            } else {
                return pte;
            }
        }

        unreachable!("Valid non-leaf PTE at level 0");
    }

    /// Do a virtual to physical address lookup.
    pub fn virt2phys(&self, vaddr: usize) -> Virt2Phys {
        if !is_canon_addr(vaddr) {
            return Virt2Phys {
                page_vaddr: vaddr & !(PAGE_SIZE as usize - 1),
                page_paddr: 0,
                size: PAGE_SIZE as usize,
                paddr: 0,
                flags: 0,
                valid: false,
            };
        }

        let pte: PTE = self.walk(vaddr);

        let size = (PAGE_SIZE as usize) << (cpu::mmu::BITS_PER_LEVEL * pte.level as u32);
        let page_vaddr = vaddr & !(size - 1);
        let page_paddr = pte.ppn * PAGE_SIZE as usize;
        let offset = vaddr - page_vaddr;
        Virt2Phys {
            page_vaddr,
            page_paddr,
            size,
            paddr: page_paddr + offset,
            flags: pte.flags,
            valid: pte.valid,
        }
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

/// Result of a virtual to physical lookup.
#[derive(Clone, Copy)]
pub struct Virt2Phys {
    /// Virtual address of page start.
    pub page_vaddr: usize,
    /// Physical address of page start.
    pub page_paddr: PAddrr,
    /// Size of the mapping in bytes.
    pub size: usize,
    /// Physical address.
    pub paddr: usize,
    /// Flags of the mapping.
    pub flags: u32,
    /// Whether the mapping exists; if false, only `vpn` and `size` are valid.
    pub valid: bool,
}

impl Debug for Virt2Phys {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use flags::*;
        f.debug_struct("Virt2Phys")
            .field("page_vaddr", &format_args!("0x{:x}", self.page_vaddr))
            .field("page_paddr", &format_args!("0x{:x}", self.page_paddr))
            .field("size", &format_args!("0x{:x}", self.size))
            .field("paddr", &format_args!("0x{:x}", self.paddr))
            .field(
                "flags",
                &format_args!(
                    "0x{:x} /* {}{}{}{}{}{}{} {} {} */",
                    self.flags,
                    if self.flags & R != 0 { 'R' } else { '-' },
                    if self.flags & W != 0 { 'W' } else { '-' },
                    if self.flags & X != 0 { 'X' } else { '-' },
                    if self.flags & U != 0 { 'U' } else { '-' },
                    if self.flags & G != 0 { 'G' } else { '-' },
                    if self.flags & A != 0 { 'A' } else { '-' },
                    if self.flags & D != 0 { 'D' } else { '-' },
                    if self.flags & REFCOUNT != 0 {
                        "RC"
                    } else {
                        "--"
                    },
                    if self.flags & IO != 0 {
                        "IO"
                    } else if self.flags & NC != 0 {
                        "NC"
                    } else {
                        "--"
                    }
                ),
            )
            .field("valid", &self.valid)
            .finish()
    }
}

/// Get the index in the given page table level for the given virtual address.
#[inline(always)]
fn get_vpn_index(vaddr: usize, level: u8) -> usize {
    (vaddr >> (level as u32 * BITS_PER_LEVEL + PAGE_SIZE.ilog2())) % (1usize << BITS_PER_LEVEL)
}

/// Read a PTE without any fencing or flushing.
#[inline(always)]
unsafe fn read_pte(pgtable_ppn: PAddrr, index: usize) -> PackedPTE {
    let pte_vaddr = unsafe { HHDM_OFFSET } + pgtable_ppn + index * size_of::<PackedPTE>();
    unsafe { (*(pte_vaddr as *mut Atomic<PackedPTE>)).load(Ordering::Acquire) }
}

/// Write a PTE without any fencing or flushing.
#[inline(always)]
unsafe fn xchg_pte(pgtable_ppn: PAddrr, index: usize, pte: PackedPTE) -> PackedPTE {
    let pte_vaddr = unsafe { HHDM_OFFSET } + pgtable_ppn + index * size_of::<PackedPTE>();
    unsafe { (*(pte_vaddr as *mut Atomic<PackedPTE>)).swap(pte, Ordering::AcqRel) }
}

/// Compare-exchange a PTE.
#[inline(always)]
unsafe fn cmpxchg_pte(pgtable_ppn: PAddrr, index: usize, old: PackedPTE, new: PackedPTE) -> bool {
    let pte_vaddr = unsafe { HHDM_OFFSET } + pgtable_ppn + index * size_of::<PackedPTE>();
    unsafe {
        (*(pte_vaddr as *mut Atomic<PackedPTE>))
            .compare_exchange_weak(old, new, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }
}

/// Try to allocate a new page table page.
fn alloc_pgtable_page() -> EResult<PAddrr> {
    let ppn = unsafe { pmm::page_alloc(0, pmm::PageUsage::PageTable) }?;
    for i in 0..1usize << BITS_PER_LEVEL {
        unsafe { xchg_pte(ppn, i, INVALID_PTE) };
    }
    Ok(ppn)
}

/// Try to split a page table leaf node.
fn split_pgtable_leaf(orig: PTE, new_level: u8) -> EResult<PAddrr> {
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
pub fn is_canon_page(addr: usize) -> bool {
    // The upper (usually 12) bits of a VPN are ignored because a VPN is actually `usize::BITS - PAGE_SIZE.ilog2()` bits.
    let addr = (addr as isize) << PAGE_SIZE.ilog2() >> PAGE_SIZE.ilog2();
    let exp = usize::BITS - BITS_PER_LEVEL * unsafe { PAGING_LEVELS };
    let canon_page = (addr << exp) >> exp;
    canon_page == addr
}

/// Determine whether an address is a canonical kernel address.
pub fn is_canon_kernel_page(addr: usize) -> bool {
    is_canon_page(addr) && (addr as isize) << PAGE_SIZE.ilog2() < 0
}

/// Determine whether an address is a canonical user address.
pub fn is_canon_user_page(addr: usize) -> bool {
    is_canon_page(addr) && (addr as isize) >= 0
}

/// Determine whether an address is canonical.
pub fn is_canon_page_range(range: Range<usize>) -> bool {
    is_canon_page(range.start) && (range.len() == 0 || is_canon_page(range.end - 1))
}

/// Determine whether an address is a canonical kernel address.
pub fn is_canon_kernel_page_range(range: Range<usize>) -> bool {
    is_canon_kernel_page(range.start) && (range.len() == 0 || is_canon_kernel_page(range.end - 1))
}

/// Determine whether an address is a canonical user address.
pub fn is_canon_user_page_range(range: Range<usize>) -> bool {
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
pub fn higher_half_page() -> usize {
    higher_half_vaddr() / PAGE_SIZE as usize
}

/// Get the start of the higher half.
pub fn higher_half_vaddr() -> usize {
    canon_half_size().wrapping_neg()
}
