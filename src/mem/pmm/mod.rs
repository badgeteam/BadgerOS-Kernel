// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ops::{Range, Sub},
    ptr::slice_from_raw_parts_mut,
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
};

use crate::{
    badgelib::irq::IrqGuard,
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
        spinlock::Spinlock,
    },
    config::PAGE_SIZE,
    mem::vmm::{self, HHDM_OFFSET},
};

mod c_api;
pub mod phys_box;
pub mod phys_ptr;

/// Kinds of usage for pages of memory.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageUsage {
    /// Dummy entry for unusable page.
    Unusable = 0,
    /// Unused page.
    Free,
    /// Part of a page table.
    PageTable,
    /// Contains cached data.
    Cache,
    /// Part of a mmap'ed file.
    Mmap,
    /// Anonymous user memory.
    UserAnon,
    /// Anonymous kernel memory.
    KernelAnon,
    /// Kernel slabs memory (may be removed in the future).
    KernelSlab,
    /// The actual kernel executable itself, be that code or data.
    KernelSegment,
}

/// Physical memory page metadata.
#[repr(C)]
pub struct Page {
    /// Page refcount, may be used for arbitrary purposes by the owner.
    /// In virtual memory objects, this counts the number of times a page is mapped in a pmap.
    pub refcount: AtomicU32,
    /// Order of the buddy block this page belongs to.
    order: u8,
    /// Current page usage.
    usage: PageUsage,
    // TODO: Pointer to structure that exposes where it's mapped in user virtual memory.
    // Kernel virtual mappings need not be tracked because they are not swappable.
}

impl Page {
    /// Get the physical address of this page from its metadata pointer.
    pub fn paddr(&self) -> PAddrr {
        unsafe {
            let vaddr = self as *const Self as usize;
            (vaddr
                .wrapping_sub(PAGE_STRUCTS_PADDR.wrapping_mul(PAGE_SIZE as usize))
                .wrapping_sub(vmm::HHDM_OFFSET)
                / size_of::<Page>())
            .wrapping_sub(PAGE_RANGE.start)
                * PAGE_SIZE as usize
        }
    }
    /// Get the buddy alloc page order.
    pub fn order(&self) -> u8 {
        self.order
    }
    /// Get the buddy alloc page order.
    pub fn usage(&self) -> PageUsage {
        self.usage
    }
}

/// Physical memory freelist link.
#[derive(Clone, Copy)]
struct FreeListLink {
    prev: PAddrr,
    next: PAddrr,
}

/// Unsigned integer that can store a physical address.
pub type PAddrr = usize;

/// Maximum order that blocks can be coalesced into.
pub const MAX_ORDER: u8 = 64;

/// Total physical pages.
pub static TOTAL_PAGES: AtomicUsize = AtomicUsize::new(0);
/// Physical pages that are used by the kernel.
pub static KERNEL_PAGES: AtomicUsize = AtomicUsize::new(0);
/// Physical pages that are caches (can be reclaimed if low on free pages).
pub static CACHE_PAGES: AtomicUsize = AtomicUsize::new(0);
/// Physical pages that are unused.
pub static FREE_PAGES: AtomicUsize = AtomicUsize::new(0);
/// Physical pages used in use.
pub static USED_PAGES: AtomicUsize = AtomicUsize::new(0);

/// Range of page numbers covered by the page allocator.
static mut PAGE_RANGE: Range<usize> = 0..0;
pub fn page_range() -> Range<usize> {
    unsafe { PAGE_RANGE.start..PAGE_RANGE.end }
}
/// Pointer to the page struct array.
static mut PAGE_STRUCTS_PADDR: PAddrr = 0;
/// Free lists per buddy order.
static FREE_LIST: Spinlock<[PAddrr; MAX_ORDER as usize]> =
    unsafe { Spinlock::new_static([PAddrr::MAX; MAX_ORDER as usize]) };

/// Calculates the minimum sized order that will fit this many bytes.
pub const fn size_to_order(byte_size: usize) -> u8 {
    debug_assert!(byte_size > 0);
    let pages = byte_size.div_ceil(PAGE_SIZE as usize) as usize;
    (usize::BITS - (pages - 1).leading_zeros()) as u8
}

/// Calculates how many bytes are in a block of a certain order.
pub const fn order_to_size(order: u8) -> usize {
    debug_assert!(order < MAX_ORDER);
    (PAGE_SIZE as usize) << order
}

/// Calculates the minimum sized order that will fit this many pages.
pub const fn pages_to_order(pages: usize) -> u8 {
    debug_assert!(pages > 0);
    (usize::BITS - (pages - 1).leading_zeros()) as u8
}

/// Calculates how many pages are in a block of a certain order.
pub const fn order_to_pages(order: u8) -> usize {
    debug_assert!(order < MAX_ORDER);
    1 << order
}

/// Helper function that gets the freelist link for a block, assuming it is free.
unsafe fn free_list_struct(paddr: PAddrr) -> *mut FreeListLink {
    debug_assert!(paddr % PAGE_SIZE as usize == 0);
    (paddr + unsafe { vmm::HHDM_OFFSET }) as *mut FreeListLink
}

/// Unlink a page from its freelist.
unsafe fn free_list_unlink(block: PAddrr, list_head: &mut PAddrr) {
    debug_assert!(unsafe { free_list_contains(block, *list_head) });
    let link = unsafe { *free_list_struct(block) };
    debug_assert!(link.prev != block);
    debug_assert!(link.next != block);

    if link.next != PAddrr::MAX {
        let next_link = unsafe { &mut *free_list_struct(link.next) };
        next_link.prev = link.prev;
        debug_assert!(next_link.prev != link.next);
        debug_assert!(next_link.next != link.next);
    }

    if link.prev != PAddrr::MAX {
        let prev_link = unsafe { &mut *free_list_struct(link.prev) };
        prev_link.next = link.next;
        debug_assert!(prev_link.next != link.prev);
        debug_assert!(prev_link.prev != link.prev);
    } else {
        *list_head = link.next;
    }
    debug_assert!(!unsafe { free_list_contains(block, *list_head) });
}

/// Link a page into a freelist.
unsafe fn free_list_link(block: PAddrr, list_head: &mut PAddrr) {
    debug_assert!(!unsafe { free_list_contains(block, *list_head) });
    let link = unsafe { &mut *free_list_struct(block) };

    link.next = *list_head;
    link.prev = PAddrr::MAX;
    if link.next != PAddrr::MAX {
        let next_link = unsafe { &mut *free_list_struct(link.next) };
        next_link.prev = block;
        debug_assert!(next_link.prev != link.next);
        debug_assert!(next_link.next != link.next);
    }

    *list_head = block;

    debug_assert!(link.prev != block);
    debug_assert!(link.next != block);
    debug_assert!(unsafe { free_list_contains(block, *list_head) });
}

/// Check whether a page is in a certain freelist.
unsafe fn free_list_contains(block: PAddrr, list_head: PAddrr) -> bool {
    debug_assert!(block % PAGE_SIZE as usize == 0);
    debug_assert!(unsafe {
        PAGE_RANGE.start <= (block / PAGE_SIZE as usize)
            && (block / PAGE_SIZE as usize) < PAGE_RANGE.end
    });
    let mut cur_node = list_head;
    while cur_node != PAddrr::MAX {
        debug_assert!(unsafe {
            PAGE_RANGE.start <= (cur_node / PAGE_SIZE as usize)
                && (cur_node / PAGE_SIZE as usize) < PAGE_RANGE.end
        });
        if cur_node == block {
            return true;
        }
        let link = unsafe { *free_list_struct(cur_node) };
        cur_node = link.next;
    }
    false
}

/// Allocate `1 << order` pages of physical memory.
pub unsafe fn page_alloc(order: u8, usage: PageUsage) -> EResult<PAddrr> {
    debug_assert!(order < MAX_ORDER);
    debug_assert!(usage != PageUsage::Unusable && usage != PageUsage::Free);
    let _noirq = IrqGuard::new();
    let mut free_list = FREE_LIST.lock();

    // Determine order to split at.
    let mut split_order = order;
    while free_list[split_order as usize] == PAddrr::MAX {
        if split_order >= MAX_ORDER - 1 {
            logkf!(
                LogLevel::Error,
                "Out of memory (allocating block of order {})",
                order
            );
            return Err(Errno::ENOMEM);
        }
        split_order += 1;
    }

    // Split blocks until one of the desired order is created.
    for split_order in (order + 1..split_order + 1).rev() {
        let block = free_list[split_order as usize];
        // Not asserting block order here because it might be temporarily out of date.
        debug_assert!({
            let shift = split_order + PAGE_SIZE.ilog2() as u8;
            block == block >> shift << shift
        });
        unsafe { free_list_unlink(block, &mut free_list[split_order as usize]) };
        // Upper half of block gets the order changed because the lower half will be alloc'ed anyway
        unsafe {
            let meta = page_struct(block);
            (*meta).usage = PageUsage::Free;
            (*meta).order = order as u8;
        }
        unsafe {
            // Do not reorder these inserts.
            free_list_link(
                block + ((PAGE_SIZE as usize) << (split_order - 1)),
                &mut free_list[split_order as usize - 1],
            );
            free_list_link(block, &mut free_list[split_order as usize - 1]);
        }
    }

    // Mark block as in use.
    let block = free_list[order as usize];
    if block == PAddrr::MAX {
        logkf!(
            LogLevel::Error,
            "Out of memory (allocating block of order {})",
            order
        );
        return Err(Errno::ENOMEM);
    }
    unsafe { free_list_unlink(block, &mut free_list[order as usize]) };
    let mut page_meta = page_struct(block);
    for _ in 0..1 << order {
        unsafe {
            (*page_meta).usage = usage;
            (*page_meta).order = order as u8;
            (*page_meta).refcount = AtomicU32::new(0);
        }
        page_meta = page_meta.wrapping_add(1);
    }

    drop(free_list);

    // Account memory usage.
    FREE_PAGES.fetch_sub(1 << order, Ordering::Relaxed);
    match usage {
        PageUsage::Free | PageUsage::Unusable | PageUsage::KernelSegment => {
            panic!(
                "Inalid page usage {:#?} for newly allocated block 0x{:x}",
                usage, block
            )
        }
        PageUsage::Cache => {
            CACHE_PAGES.fetch_add(1 << order, Ordering::Relaxed);
        }
        PageUsage::PageTable | PageUsage::KernelAnon | PageUsage::KernelSlab => {
            KERNEL_PAGES.fetch_add(1 << order, Ordering::Relaxed);
            USED_PAGES.fetch_add(1 << order, Ordering::Relaxed);
        }
        PageUsage::Mmap | PageUsage::UserAnon => {
            USED_PAGES.fetch_add(1 << order, Ordering::Relaxed);
        }
    }

    Ok(block)
}

/// Get the [`Page`] struct for some physical address.
/// Manipulating the data within the struct is unsafe.
pub fn page_struct(paddr: PAddrr) -> *mut Page {
    debug_assert!(paddr % PAGE_SIZE as usize == 0);
    unsafe {
        let ppn = paddr / PAGE_SIZE as usize;
        debug_assert!(PAGE_RANGE.start <= ppn && ppn < PAGE_RANGE.end);
        let vaddr = PAGE_STRUCTS_PADDR
            .wrapping_add(vmm::HHDM_OFFSET)
            .wrapping_add(
                ppn.wrapping_sub(PAGE_RANGE.start)
                    .wrapping_mul(size_of::<Page>()),
            );
        vaddr as *mut Page
    }
}

/// Get the [`Page`] struct for the start of the block that some physical address lies in.
/// Manipulating the data within the struct is unsafe.
pub fn page_struct_base(paddr: PAddrr) -> (*mut Page, u8) {
    debug_assert!(paddr % PAGE_SIZE as usize == 0);
    unsafe {
        let meta = page_struct(paddr);
        let order = (*meta).order();
        let shift = order + PAGE_SIZE.ilog2() as u8;
        let aligned_paddr = paddr >> shift << shift;
        (meta.wrapping_sub(paddr.wrapping_sub(aligned_paddr) / PAGE_SIZE as usize), order)
    }
}

/// Mark a single block of arbitrary order as free.
pub unsafe fn page_free(mut block: PAddrr, mut order: u8) {
    debug_assert!(block % ((PAGE_SIZE as usize) << order) == 0);
    let pages_freed: PAddrr = 1 << order;
    let _noirq = IrqGuard::new();
    let mut free_list = FREE_LIST.lock();

    // Remove the pages from where they were previously accounted.
    match unsafe { &*page_struct(block) }.usage() {
        PageUsage::Free => unreachable!("Unused page marked as free again"),
        PageUsage::KernelSegment => unreachable!("Kernel segment page marked as free"),
        PageUsage::Unusable => (), // Not accounted as being used for something.
        PageUsage::Cache => {
            CACHE_PAGES.fetch_sub(pages_freed, Ordering::Relaxed);
        }
        PageUsage::PageTable | PageUsage::KernelAnon | PageUsage::KernelSlab => {
            KERNEL_PAGES.fetch_sub(pages_freed, Ordering::Relaxed);
            USED_PAGES.fetch_sub(pages_freed, Ordering::Relaxed);
        }
        PageUsage::Mmap | PageUsage::UserAnon => {
            USED_PAGES.fetch_sub(pages_freed, Ordering::Relaxed);
        }
    }

    /// Try to coalesce a block with its buddy.
    fn try_coalesce(
        block: PAddrr,
        order: u8,
        free_list: &mut [PAddrr; MAX_ORDER as usize],
    ) -> bool {
        // Determine whether coalescing is possible.
        let buddy = block ^ ((PAGE_SIZE as usize) << order);
        let buddy_ppn = buddy / PAGE_SIZE as usize;
        if !unsafe { PAGE_RANGE.start <= buddy_ppn && buddy_ppn < PAGE_RANGE.end } {
            return false;
        }
        let buddy_page = unsafe { &*page_struct(buddy) };
        if buddy_page.usage != PageUsage::Free || buddy_page.order() != order {
            return false;
        }

        // Remove buddy from freelist.
        unsafe {
            free_list_unlink(buddy, &mut free_list[order as usize]);
        }

        true
    }

    // Attempt to coalesce.
    while order < MAX_ORDER && try_coalesce(block, order, &mut free_list) {
        block &= !((PAGE_SIZE as usize) << order);
        order += 1;
    }

    // Set the pages up for future usage.
    let mut page_meta = page_struct(block);
    for _ in block..block + (1 << order) {
        unsafe {
            (*page_meta).usage = PageUsage::Free;
            (*page_meta).order = order;
        }
        page_meta = page_meta.wrapping_add(1);
    }

    // Insert it into the freelist.
    unsafe { free_list_link(block, &mut free_list[order as usize]) };

    // Account the memory as free.
    FREE_PAGES.fetch_add(pages_freed, Ordering::Relaxed);
}

/// Mark a range of blocks as free.
pub unsafe fn mark_free(mut memory: Range<PAddrr>) {
    debug_assert!(memory.start % PAGE_SIZE as usize == 0);
    debug_assert!(memory.end % PAGE_SIZE as usize == 0);
    while memory.end > memory.start {
        // Max order of the page depends on physical address and available space.
        let max_order = memory
            .start
            .trailing_zeros()
            .sub(PAGE_SIZE.ilog2())
            .min((memory.end - memory.start).ilog2()) as u8;
        unsafe { page_free(memory.start, max_order) };
        memory.start += (PAGE_SIZE as usize) << max_order;
    }
}

/// Initialize the physical memory allocator.
/// It is assumed that the boot protocol implementation hereafter marks the kernel executable with [`PageUsage::KernelSegment`].
pub unsafe fn init(total: Range<PAddrr>, early: Range<PAddrr>) {
    debug_assert!(total.start % PAGE_SIZE as usize == 0);
    debug_assert!(total.end % PAGE_SIZE as usize == 0);
    debug_assert!(early.start % PAGE_SIZE as usize == 0);
    debug_assert!(early.end % PAGE_SIZE as usize == 0);
    unsafe {
        TOTAL_PAGES.store(total.end - total.start, Ordering::Relaxed);
        PAGE_RANGE = total.start / PAGE_SIZE as usize..total.end / PAGE_SIZE as usize;
        // How many pages will be used by the page metadata structs.
        let meta_pages = (size_of::<Page>() * total.len()).div_ceil(PAGE_SIZE as usize) as PAddrr;
        // There needs to be at least a small amount of available pages to bootstrap MM.
        if early.end - early.start < meta_pages + 64 {
            panic!("Insufficient memory");
        }
        PAGE_STRUCTS_PADDR = early.start;

        // Mark all pages as unusable...
        for page in total.step_by(PAGE_SIZE as usize) {
            let page_struct = page_struct(page);
            *page_struct = core::mem::zeroed();
        }

        // ...but mark the early pool as free.
        mark_free(early.start + meta_pages * PAGE_SIZE as usize..early.end);
    }
}

pmm_ktest!(PMM_BASIC, unsafe {
    // Allocate one page of a couple orders.
    let mut ppn = [0; 4];
    for order in 0..3 {
        ppn[order] = page_alloc(order as u8, PageUsage::KernelAnon)?;
    }
    // Make sure we didn't get duplicates.
    for x in 1..3 {
        for y in 0..x {
            ktest_expect!(ppn[x], !=, ppn[y], [x, y]);
        }
    }
    // Ensure metadata for alloc'ed blocks is ok.
    for order in 0..3 {
        let page_meta = &*page_struct(ppn[order]);
        ktest_expect!(page_meta.usage, PageUsage::KernelAnon);
        ktest_assert!(!free_list_contains(ppn[order], FREE_LIST.lock()[order]));
    }
    // Free pages again.
    for order in 0..3 {
        page_free(ppn[order], order as u8);
    }
});

pmm_ktest!(PMM_MANYPAGES, unsafe {
    let l2_paddr = page_alloc(0, PageUsage::KernelAnon)?;

    let l2 = &mut *slice_from_raw_parts_mut((l2_paddr + HHDM_OFFSET) as *mut PAddrr, 32);

    let old_avl = FREE_PAGES.load(Ordering::Relaxed);

    for x in 0..l2.len() {
        l2[x] = page_alloc(0, PageUsage::KernelAnon)?;
        let l1 = &mut *slice_from_raw_parts_mut((l2[x] + vmm::HHDM_OFFSET) as *mut PAddrr, 512);
        for y in 0..l1.len() {
            l1[y] = page_alloc(0, PageUsage::KernelAnon)?;
            for z in 0..y {
                ktest_expect!(l1[z], !=, l1[y]);
            }
        }
    }

    for x in 0..l2.len() {
        let l1 = &mut *slice_from_raw_parts_mut((l2[x] + vmm::HHDM_OFFSET) as *mut PAddrr, 512);
        for y in 0..l1.len() {
            page_free(l1[y], 0);
        }
        page_free(l2[x], 0);
    }

    ktest_expect!(FREE_PAGES.load(Ordering::Relaxed), old_avl);
});
