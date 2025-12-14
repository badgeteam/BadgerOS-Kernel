// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    ffi::c_void,
    fmt::Debug,
    ops::Range,
    ptr::{slice_from_raw_parts, slice_from_raw_parts_mut},
    sync::atomic::AtomicUsize,
};

use alloc::vec::Vec;
use pagetable::{OwnedPTE, PAGING_LEVELS, canon_half_pages};

use crate::{
    badgelib::{irq::IrqGuard, rcu},
    bindings::{error::EResult, log::LogLevel, mutex::Mutex, raw::memcpy},
    config::PAGE_SIZE,
    cpu::{
        self,
        mmu::{self, BITS_PER_LEVEL},
    },
    mem::{
        pmm::{self, PPN, PageUsage},
        vmm::{pagetable::PageTable, vma_alloc::VmaAlloc},
    },
};

use super::pmm::phys_ptr::PhysPtr;

mod c_api;
pub mod pagetable;
pub mod vma_alloc;

pub mod flags {
    pub use crate::cpu::mmu::flags::*;

    /// Map memory as read-write.
    pub const RW: u32 = R | W;
    /// Map memory as read-execute.
    pub const RX: u32 = R | X;
    /// Map memory as read-write-execute.
    pub const RWX: u32 = R | W | X;

    /// Allow creation of I/O PTE even though the page may be RAM.
    pub(super) const HHDM: u32 = 1 << 30;
    /// Implicitly create a demand-paged mapping.
    pub const LAZY: u32 = 1 << 31;
}

/// Unsigned integer that can store a virtual page number.
pub type AtomicVPN = AtomicUsize;

/// Unsigned integer that can store a virtual page number.
pub type VPN = usize;

/// Naturally aligned slice that is a page or more of zeroes.
static mut ZEROES: *const [u8] = unsafe { core::mem::zeroed() };

/// Cache of the physical page number of [`ZEROES`].
static mut ZEROES_PPN: PPN = 0;

/// The kernel memory map.
static mut KERNEL_MM: UnsafeCell<Option<Memmap>> = UnsafeCell::new(None);

/// Get the kernel memory map.
pub fn kernel_mm() -> &'static Memmap {
    unsafe { (*&raw const KERNEL_MM).as_ref_unchecked() }
        .as_ref()
        .unwrap()
}

unsafe extern "C" {
    static __start_text: [u8; 0];
    static __stop_text: [u8; 0];
    static __start_rodata: [u8; 0];
    static __stop_rodata: [u8; 0];
    static __start_data: [u8; 0];
    static __stop_data: [u8; 0];

    /// Higher-half direct map virtual address.
    /// Provided by boot protocol.
    #[link_name = "vmm_hhdm_vaddr"]
    pub static mut HHDM_VADDR: usize;
    /// Higher-half direct map address offset (paddr -> vaddr).
    /// Provided by boot protocol.
    #[link_name = "vmm_hhdm_offset"]
    pub static mut HHDM_OFFSET: usize;
    /// Higher-half direct map size.
    /// Provided by boot protocol.
    #[link_name = "vmm_hhdm_size"]
    pub static mut HHDM_SIZE: usize;
    /// Kernel base virtual address.
    /// Provided by boot protocol.
    #[link_name = "vmm_kernel_vaddr"]
    pub static mut KERNEL_VADDR: usize;
    /// Kernel base physical address.
    /// Provided by boot protocol.
    #[link_name = "vmm_kernel_paddr"]
    pub static mut KERNEL_PADDR: usize;
}

/// High-level interface to memory maps.
#[repr(C)]
pub struct Memmap {
    is_kernel: bool,
    /// An invalid non-NULL leaf PTE is used for swapped-out or mmap()ed pages.
    pagetable: PageTable,
    vma_alloc: Mutex<VmaAlloc>,
    // TODO: Metadata storage for e.g. file mmap()s.
    // TODO: Support for valid PTEs that are PROT_NONE.
}

unsafe impl Send for Memmap {}
unsafe impl Sync for Memmap {}

impl Memmap {
    pub fn root_ppn(&self) -> PPN {
        self.pagetable.root_ppn()
    }

    /// Create a new user memory map.
    pub fn new_user() -> EResult<Self> {
        let mut pagetable = PageTable::new()?;
        let kernel_mm = kernel_mm();
        unsafe { pagetable.copy_higher_half(&kernel_mm.pagetable) };
        Ok(Self {
            is_kernel: false,
            pagetable,
            vma_alloc: Mutex::new(VmaAlloc::new(canon_half_pages() / 2..canon_half_pages())?),
        })
    }

    /// Do a virtual to physical lookup in this context.
    pub fn virt2phys(&self, vaddr: usize) -> Virt2Phys {
        if !pagetable::is_canon_addr(vaddr) {
            logkf!(
                LogLevel::Warning,
                "Tried to look up non-canonical virtual address"
            );
            return Virt2Phys {
                page_vaddr: vaddr & !(PAGE_SIZE as usize - 1),
                page_paddr: 0,
                size: PAGE_SIZE as usize,
                paddr: 0,
                flags: 0,
                valid: false,
            };
        }

        let pte = self.pagetable.walk(vaddr / PAGE_SIZE as usize);

        let size = (PAGE_SIZE as usize) << (mmu::BITS_PER_LEVEL * pte.order as u32);
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

    /// Create a fork()ed duplicate of this map.
    /// Converts anonymous writeable mappings into copy-on-write mappings.
    pub fn fork(&self) -> EResult<Self> {
        assert!(!self.is_kernel);
        // This function doesn't change the effective contents of this memmap, but must ensure no concurrent modifications happen;
        // it cannot lock the page table spinlock constantly, so this guard effectively prevents concurrent modifications.
        let guard = self.vma_alloc.lock();

        let new_mm = Self::new_user()?;

        let mut min_vpn = 0;
        while let Some((vpn, mut pte)) = self.pagetable.find_first(min_vpn, false) {
            if !pagetable::is_canon_user_page(vpn) {
                break;
            }

            if pte.valid && pte.flags & (flags::W | flags::MODE) == flags::W {
                // If PTE is writeable anonymous memory, it must be turned into a CoW mapping.
                pte.flags = (pte.flags & !flags::W) | flags::COW;
                unsafe { self.pagetable.map(vpn, OwnedPTE::from_raw_ref(pte)) }.unwrap();
            }

            if !pte.is_null() {
                unsafe { new_mm.pagetable.map(vpn, OwnedPTE::from_raw_ref(pte)) }?;
            }

            min_vpn = vpn + (1 << pte.order);
        }

        // TODO: Fallible clone of this structure.
        *new_mm.vma_alloc.lock() = guard.clone();

        Ok(new_mm)
    }

    // TODO: Mapping function for files.

    /// Reserve a range of memory without mapping anything.
    pub fn reserve(&self, vpn: Option<VPN>, size: VPN) -> EResult<VPN> {
        if let Some(vpn) = vpn {
            self.vma_alloc.lock().steal(vpn..vpn + size);
            Ok(vpn)
        } else {
            self.vma_alloc.lock().alloc(size)
        }
    }

    /// Create a new mapping at a fixed physical address.
    /// Assumes that the range encompases existing physical memory.
    ///
    /// The [`flags::MODE`] flags affect how the mapping is treated:
    /// - `0`: Anonymous mapping; writable mappings will be turned into CoW on [`Self::fork`].
    /// - [`flags::COW`]: Page mustn't be writable; it will be copied on write.
    /// - [`flags::SHM`]: Writable shared mappings won't be turned into CoW on [`Self::fork`].
    /// - [`flags::MMIO`]: `ppn` is assumed not to be normal RAM.
    ///
    /// If `vpn` is [`None`], an arbitrary virtual address range is chosen.
    pub unsafe fn map_fixed(
        &self,
        ppn: PPN,
        vpn: Option<VPN>,
        size: VPN,
        mut flags: u32,
    ) -> EResult<VPN> {
        let mut guard = self.vma_alloc.lock();
        let mut do_steal = false;
        let vpn = match vpn {
            Some(vpn) => {
                do_steal = true;
                vpn
            }
            None => guard.alloc(size)?,
        };

        if self.is_kernel {
            flags |= flags::A | flags::D | flags::G;
            assert!(pagetable::is_canon_kernel_page_range(vpn..vpn + size));
        } else {
            flags |= flags::A | flags::D | flags::U;
            assert!(flags & flags::G == 0);
            assert!(pagetable::is_canon_user_page_range(vpn..vpn + size));
        }

        let mut ptes = Vec::new();
        // Storing the to-be-freed pages here so they're only freed after an RCU sync.
        let mut to_free = Vec::new();
        to_free.try_reserve(size)?;

        let mut offset = 0;
        while offset < size {
            let order = pagetable::calc_superpage(vpn + offset, ppn + offset, size - offset);
            if flags & flags::MODE == flags::MMIO {
                ptes.push(OwnedPTE::new_io(ppn + offset, order, flags));
            } else {
                let mem = unsafe { PhysPtr::from_ref_ppn(ppn + offset) };
                let mem_offset = ppn + offset - mem.ppn();
                ptes.push(OwnedPTE::new_ram(mem, mem_offset, order, flags));
            }
            debug_assert!(ptes.last().as_ref().unwrap().order() == order);
            offset += 1 << mmu::BITS_PER_LEVEL * order as u32;
        }

        unsafe { self.map_impl(ptes, &mut to_free, vpn, size)? };
        if do_steal {
            guard.steal(vpn..vpn + size);
        }

        if to_free.len() != 0 {
            rcu::rcu_sync();
        }

        Ok(vpn)
    }

    /// Allocate memory for and create a new mapping.
    /// Assumes that the range encompases existing physical memory.
    ///
    /// The [`flags::MODE`] flags affect how the mapping is treated:
    /// - `0`: Anonymous mapping; writable mappings will be turned into CoW on [`Self::fork`].
    /// - [`flags::COW`]: Page mustn't be writable; it will be copied on write.
    /// - [`flags::SHM`]: Writable shared mappings won't be turned into CoW on [`Self::fork`].
    /// - [`flags::MMIO`]: Invalid for this function.
    ///
    /// If `vpn` is [`None`], an arbitrary virtual address range is chosen.
    pub unsafe fn map_ram(&self, vpn: Option<VPN>, size: VPN, mut flags: u32) -> EResult<VPN> {
        let mut guard = self.vma_alloc.lock();
        let mut do_steal = false;
        let vpn = match vpn {
            Some(vpn) => {
                do_steal = true;
                vpn
            }
            None => guard.alloc(size)?,
        };

        if self.is_kernel {
            flags |= flags::A | flags::D | flags::G;
            assert!(pagetable::is_canon_kernel_page_range(vpn..vpn + size));
        } else {
            flags |= flags::A | flags::D | flags::U;
            assert!(flags & flags::G == 0);
            assert!(pagetable::is_canon_user_page_range(vpn..vpn + size));
        }

        assert!(flags & flags::MODE != flags::MMIO);
        let mut ptes = Vec::new();
        // Storing the to-be-freed pages here so they're only freed after an RCU sync.
        let mut to_free = Vec::new();
        to_free.try_reserve(size)?;

        if flags & (flags::LAZY | flags::W) == flags::LAZY | flags::W {
            // Demand paging implemented by doing CoW of the page of zeroes.
            let zeroes = unsafe { PhysPtr::from_ref_ppn(ZEROES_PPN) };
            ptes.reserve_exact(size);
            for _ in 0..size {
                ptes.push(OwnedPTE::new_ram(
                    zeroes.clone(),
                    0,
                    0,
                    (flags & !flags::W) | flags::COW,
                ))
            }
        } else {
            // Immediately map memory.
            let mem = PhysPtr::new(
                (VPN::BITS - size.leading_zeros() - 1) as u8,
                pmm::PageUsage::KernelAnon,
            )?;
            let mut offset = 0;
            while offset < size {
                let order =
                    pagetable::calc_superpage(vpn + offset, mem.ppn() + offset, size - offset);
                ptes.push(OwnedPTE::new_ram(mem.clone(), offset, order, flags));
                offset += 1 << mmu::BITS_PER_LEVEL * order as u32;
            }
        }

        unsafe { self.map_impl(ptes, &mut to_free, vpn, size)? };
        if do_steal {
            guard.steal(vpn..vpn + size);
        }

        rcu::rcu_sync();

        Ok(vpn)
    }

    /// Change the protection attributes for a region.
    pub unsafe fn protect(&self, vpn: VPN, new_flags: u32) -> EResult<()> {
        // Inhibit concurrent changes to the mappings.
        let _guard = self.vma_alloc.lock();

        todo!()
    }

    /// Common implementation of all mapping functions.
    unsafe fn map_impl(
        &self,
        ptes: Vec<OwnedPTE>,
        to_free: &mut Vec<OwnedPTE>,
        vpn: VPN,
        size: VPN,
    ) -> EResult<()> {
        let actual_size = ptes
            .iter()
            .map(|x| 1 << x.order() as u32 * BITS_PER_LEVEL)
            .sum::<VPN>();
        debug_assert!(
            actual_size == size,
            "map_impl size mismatch; expected 0x{:x}, got 0x{:x}",
            size,
            actual_size
        );

        // Pre-allocate page tables so no partial mapping is left on failure.
        let mut i = 0usize;
        let mut offset = 0 as VPN;
        while offset < size {
            self.pagetable.prealloc(vpn + offset, ptes[i].order())?;
            offset += 1 << mmu::BITS_PER_LEVEL * ptes[i].order() as u32;
            i += 1;
        }

        offset = 0;
        for pte in ptes {
            let level = pte.order();
            let unmapped = unsafe { self.pagetable.map(vpn + offset, pte) }
                .expect("Page tables weren't preallocated");
            if unmapped.owns_ram() {
                to_free.push(unmapped);
            }
            offset += 1 << mmu::BITS_PER_LEVEL * level as u32;
        }

        mmu::vmem_fence(None, None);

        Ok(())
    }

    /// Unmap a range of pages.
    pub unsafe fn unmap(&self, pages: Range<PPN>) {
        if self.is_kernel {
            assert!(pagetable::is_canon_kernel_page_range(pages.clone()));
        } else {
            assert!(pagetable::is_canon_user_page_range(pages.clone()));
        }

        let mut guard = self.vma_alloc.lock();
        // Storing the to-be-freed pages here so they're only freed after an RCU sync.
        let mut to_free = Vec::new();

        unsafe { self.unmap_impl(pages.clone(), &mut to_free) };

        mmu::vmem_fence(None, None);

        if to_free.len() != 0 {
            rcu::rcu_sync();
        }

        guard.free(pages);
        drop(guard);
    }

    /// Unmap a range of pages; the caller must guarantee that the contents of `to_free` are only dropped after an [`rcu::rcu_sync`].
    unsafe fn unmap_impl(&self, pages: Range<PPN>, to_free: &mut Vec<OwnedPTE>) {
        let mut vpn = pages.start;
        while vpn < pages.end {
            let pte = self.pagetable.walk(vpn);

            if !pte.is_null() {
                let old_pte = unsafe { self.pagetable.map(vpn, OwnedPTE::NULL) }.unwrap();
                if old_pte.owns_ram() {
                    to_free.push(old_pte);
                }
            }

            vpn += 1 << mmu::BITS_PER_LEVEL * pte.order as u32;
        }
    }

    /// Clear all mappings from this user memory map.
    pub unsafe fn clear(&self) {
        assert!(!self.is_kernel);
        let mut guard = self.vma_alloc.lock();
        guard.free(canon_half_pages() / 2..canon_half_pages());
        unsafe { self.pagetable.clear_lower_half() };
    }

    /// Check page faults for lazy mappings given required access permissions `access`.
    /// Called when a page fault happens on this memmap.
    /// Returns `true` if the fault should be ignored and the faulting operation retried.
    pub fn page_fault(&self, vpn: VPN, access: u32) -> bool {
        // Inhibit concurrent changes to the mappings.
        let _guard = self.vma_alloc.lock();

        let mapping = self.pagetable.walk(vpn);
        if mapping.flags & access == access {
            // TLB was likely outdated; retry.
            cpu::mmu::vmem_fence(Some(vpn * PAGE_SIZE as usize), None);
            return true;
        }

        if mapping.valid && mapping.flags & flags::COW != 0 && access == flags::W {
            // Write access to copy-on-write page.
            let res: EResult<()> = try {
                let page = PhysPtr::new(0, PageUsage::UserAnon)?;
                unsafe {
                    let ppn = page.ppn();
                    let old = self.pagetable.map(
                        vpn,
                        OwnedPTE::new_ram(page, 0, 0, (mapping.flags & !flags::COW) | flags::W),
                    )?;

                    // Copy contents of the old page.
                    memcpy(
                        (ppn * PAGE_SIZE as usize + HHDM_OFFSET) as *mut c_void,
                        (old.ppn() * PAGE_SIZE as usize + HHDM_OFFSET) as *const c_void,
                        PAGE_SIZE as usize,
                    );

                    // Ensure other threads aren't referencing the stale mapping before dropping it.
                    cpu::mmu::vmem_fence(Some(vpn * PAGE_SIZE as usize), None);
                    rcu::rcu_sync();
                    drop(old);
                }
            };
            // If successfully updated, retry the operation.
            return res.is_ok();
        }

        false
    }
}

impl Drop for Memmap {
    fn drop(&mut self) {
        assert!(!self.is_kernel);
    }
}

/// Describes the result of a virtual to physical address translation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Virt2Phys {
    /// Virtual address of page start.
    pub page_vaddr: VPN,
    /// Physical address of page start.
    pub page_paddr: PPN,
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
                    if self.flags & COW != 0 { "COW" } else { "---" },
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

/// Naturally aligned slice that is a page or more of zeroes.
pub fn zeroes() -> &'static [u8] {
    unsafe { &*ZEROES }
}

/// Initialize the virtual memory subsystem.
pub unsafe fn init() {
    unsafe {
        mmu::early_init();
        logkf!(LogLevel::Info, "MMU paging levels: {}", { PAGING_LEVELS });

        // Prepare new page tables containing kernel and HHDM.
        // Note: Using the MMIO mode to map the kernel here, as it will never be in memory marked as RAM.
        // Don't confuse this for the IO and NC flags, which actually change how the CPU accesses the pages.
        let res: EResult<()> = try {
            let tmp = &mut *(*&raw const KERNEL_MM).as_mut_unchecked();
            *tmp = Some(Memmap {
                is_kernel: true,
                pagetable: PageTable::new()?,
                vma_alloc: Mutex::new(VmaAlloc::new(
                    // 1/4 through 3/4 of the higher half is available for miscellaneous mappings.
                    pagetable::higher_half_vpn() + pagetable::canon_half_pages() / 4
                        ..pagetable::higher_half_vpn() + pagetable::canon_half_pages() * 3 / 4,
                )?),
            });
            let kernel_mm = kernel_mm();

            // Kernel RX.
            logkf_unlocked!(LogLevel::Debug, "Mapping kernel RX");
            let text_vaddr = &raw const __start_text as usize;
            let text_len = &raw const __stop_text as usize - &raw const __start_text as usize;
            debug_assert!(text_len % PAGE_SIZE as usize == 0);
            kernel_mm.map_fixed(
                (text_vaddr - KERNEL_VADDR + KERNEL_PADDR) / PAGE_SIZE as usize,
                Some(text_vaddr / PAGE_SIZE as usize),
                text_len / PAGE_SIZE as usize,
                flags::RX | flags::G,
            )?;

            // Kernel R.
            logkf_unlocked!(LogLevel::Debug, "Mapping kernel R");
            let rodata_vaddr = &raw const __start_rodata as usize;
            let rodata_len = &raw const __stop_rodata as usize - &raw const __start_rodata as usize;
            debug_assert!(rodata_len % PAGE_SIZE as usize == 0);
            kernel_mm.map_fixed(
                (rodata_vaddr - KERNEL_VADDR + KERNEL_PADDR) / PAGE_SIZE as usize,
                Some(rodata_vaddr / PAGE_SIZE as usize),
                rodata_len / PAGE_SIZE as usize,
                flags::R | flags::G,
            )?;

            // Kernel RW.
            logkf_unlocked!(LogLevel::Debug, "Mapping kernel RW");
            let data_vaddr = &raw const __start_data as usize;
            let data_len = &raw const __stop_data as usize - &raw const __start_data as usize;
            debug_assert!(data_len % PAGE_SIZE as usize == 0);
            kernel_mm.map_fixed(
                (data_vaddr - KERNEL_VADDR + KERNEL_PADDR) / PAGE_SIZE as usize,
                Some(data_vaddr / PAGE_SIZE as usize),
                data_len / PAGE_SIZE as usize,
                flags::RW | flags::G,
            )?;

            // HHDM RW.
            logkf_unlocked!(LogLevel::Debug, "Mapping HHDM RW");
            debug_assert!(HHDM_SIZE % PAGE_SIZE as usize == 0);
            kernel_mm.map_fixed(
                (HHDM_VADDR - HHDM_OFFSET) / PAGE_SIZE as usize,
                Some(HHDM_VADDR / PAGE_SIZE as usize),
                HHDM_SIZE / PAGE_SIZE as usize,
                flags::RW | flags::G | flags::MMIO | flags::HHDM,
            )?;

            // Page of zeroes.
            logkf_unlocked!(LogLevel::Debug, "Mapping zeroes page");
            let zeroes_order = 0;
            let zeroes_vpn = kernel_mm.map_ram(None, 1, flags::R | flags::G)?;
            ZEROES = slice_from_raw_parts(
                (zeroes_vpn * PAGE_SIZE as usize) as *const u8,
                (PAGE_SIZE as usize) << zeroes_order,
            );

            let zeroes_paddr = kernel_mm
                .virt2phys(zeroes_vpn * PAGE_SIZE as usize)
                .page_paddr;
            ZEROES_PPN = zeroes_paddr / PAGE_SIZE as usize;
            (&mut *slice_from_raw_parts_mut(
                (zeroes_paddr + HHDM_OFFSET) as *mut u8,
                PAGE_SIZE as usize,
            ))
                .fill(0);
        };
        res.expect("Failed to create inital page table");

        // Finalize MMU initialization and switch to new page table.
        logkf!(LogLevel::Info, "Switching to new page table");
        mmu::init(kernel_mm().pagetable.root_ppn());
        logkf!(LogLevel::Info, "Virtual memory management initialized");
    }
}
