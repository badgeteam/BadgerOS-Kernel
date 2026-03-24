// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{
    badgelib::irq::IrqGuard,
    bindings::error::EResult,
    config::PAGE_SIZE,
    kernel::sync::{spinlock::Spinlock, waitlist::Waitlist},
    mem::{pmm::PPN, vmm::VPN},
    util::rtree::RadixTree,
};

mod flags {
    /// Entry contains differences from the backing store.
    pub const DIRTY: u8 = 1 << 0;
    /// Entry is being written to disk.
    pub const WRITING: u8 = 1 << 1;
    /// Entry is being read from disk.
    pub const READING: u8 = 1 << 2;
    /// Entry is resident (can be read and written).
    pub const RESIDENT: u8 = 1 << 3;
}

/// Physical page number allocated (may be 0 if unallocated).
/// Status for this entry.
///
/// Sharing these limits physical addresses to 60 bits (16 TiB physical address space).
/// This trade-off is considered acceptable, given that no current hardware exceeds this limit.
/// Packing these together allows a `Spinlock<Entry>` to be a power-of-2 size.
#[derive(Default)]
struct EntryStatus([u32; 2]);

/// Metadata about a block/page in a [`PageCache`].
struct Entry {
    /// See [`EntryStatus`].
    status: Spinlock<EntryStatus>,
    /// Number of active references; entries cannot be evicted if there are references.
    refcount: AtomicU32,
}

impl EntryStatus {
    /// Physical page number allocated (may be 0 if unallocated).
    pub fn ppn(&self) -> PPN {
        let ppn_and_flags: PPN = bytemuck::cast(self.0);
        ppn_and_flags & (PPN::MAX >> 4)
    }

    /// Physical page number allocated (may be 0 if unallocated).
    pub fn set_ppn(&mut self, ppn: PPN) {
        let mut ppn_and_flags: PPN = bytemuck::cast(self.0);
        ppn_and_flags &= !(PPN::MAX >> 4);
        ppn_and_flags |= ppn;
        self.0 = bytemuck::cast(ppn_and_flags);
    }

    /// Status for this entry.
    pub fn flags(&self) -> u8 {
        let ppn_and_flags: PPN = bytemuck::cast(self.0);
        (ppn_and_flags >> (PPN::BITS - 4)) as u8
    }

    /// Status for this entry.
    pub fn set_flags(&mut self, flags: u8) {
        let mut ppn_and_flags: PPN = bytemuck::cast(self.0);
        ppn_and_flags &= PPN::MAX >> 4;
        ppn_and_flags |= (flags as PPN) << (PPN::BITS - 4);
        self.0 = bytemuck::cast(ppn_and_flags);
    }
}

/// Generic page cache implementation that translates between system page size and cached block size.
pub struct PageCache {
    /// Log-base 2 number of bytes a block is.
    block_size_exp: u8,
    /// Log-base 2 number of pages an entry is.
    entry_size_exp: u8,
    /// Note: To allow entries to be created/destroyed during disk access, an [`Entry`] can be accessed via raw pointers.
    /// To protect this, [`Entry::refcount`] must be equal to zero for it to be removed.
    pages: Spinlock<RadixTree<u64, UnsafeCell<Entry>>>,
    /// Waitlist for disk reads.
    read_waitlist: Waitlist,
    /// Waitlist for disk writes.
    write_waitlist: Waitlist,
}

impl PageCache {
    pub const fn new(block_size_exp: u8) -> Self {
        Self {
            block_size_exp,
            entry_size_exp: block_size_exp.saturating_sub(PAGE_SIZE.ilog2() as u8),
            pages: Spinlock::new(RadixTree::new()),
            read_waitlist: Waitlist::new(),
            write_waitlist: Waitlist::new(),
        }
    }

    /// Index calculation helper.
    #[inline(always)]
    fn index(&self, page: u64) -> (u64, VPN) {
        let page_size_exp = PAGE_SIZE.ilog2() as u8;
        if page_size_exp > self.block_size_exp {
            (1, 0)
        } else {
            let page_per_block = 1 << (self.block_size_exp - page_size_exp);
            (page * page_per_block, (page % page_per_block) as VPN)
        }
    }

    /// Get or create the entry for a given page.
    fn alloc_entry<'a>(&'a self, index: u64) -> EResult<*mut Entry> {
        let guard = self.pages.lock_shared();

        if let Some(entry) = guard.get(index) {
            unsafe {
                entry
                    .as_ref_unchecked()
                    .refcount
                    .fetch_add(1, Ordering::Relaxed);
                return Ok(entry.get());
            }
        }

        drop(guard);
        let mut guard = self.pages.lock();
        if guard.get(index).is_none() {
            guard.insert(
                index,
                UnsafeCell::new(Entry {
                    status: Spinlock::new(Default::default()),
                    refcount: AtomicU32::new(0),
                }),
            )?;
        }

        let entry = guard.get(index).unwrap();
        unsafe {
            entry
                .as_ref_unchecked()
                .refcount
                .fetch_add(1, Ordering::Relaxed);
            return Ok(entry.get());
        }
    }

    /// Read data into an entry from disk.
    unsafe fn read_from_disk(
        &self,
        pager: &dyn Pager,
        index: u64,
        status: &mut EntryStatus,
    ) -> EResult<()> {
        todo!()
    }

    /// Write data from an entry to disk.
    unsafe fn write_to_disk(
        &self,
        pager: &dyn Pager,
        index: u64,
        status: &mut EntryStatus,
    ) -> EResult<()> {
        todo!()
    }

    /// Get a page from the cache and increase its refcount.
    pub fn get(&self, pager: &dyn Pager, page: u64) -> EResult<PPN> {
        let (index, offset) = self.index(page);

        let _noirq = IrqGuard::new();
        let entry = self.alloc_entry(index)?;

        unsafe {
            let status = (&*entry).status.lock_shared();
            if status.flags() & flags::RESIDENT != 0 {
                return Ok(status.ppn() + offset);
            }
            drop(status);

            let mut status = (&*entry).status.lock();
            self.read_from_disk(pager, index, &mut status)?;
            Ok(status.ppn() + offset)
        }
    }

    /// Release a reference to a page, decreasing its refcount.
    pub fn put(&self, pager: &dyn Pager, page: u64) {
        todo!()
    }

    /// Mark a page as being dirty.
    pub fn mark_dirty(&self, pager: &dyn Pager, page: u64) {
        todo!()
    }

    /// Synchronize a page to disk.
    pub fn sync(&self, pager: &dyn Pager, page: u64, flush: bool) -> EResult<()> {
        todo!()
    }

    /// Synchronize all pages to disk.
    pub fn sync_all(&self, pager: &dyn Pager, flush: bool) -> EResult<()> {
        todo!()
    }
}

/// Interface for reading and writing pages (note: not locked to system page size).
pub trait Pager {
    /// Log-base 2 number of bytes a page is.
    fn page_size_exp(&self) -> u8;

    /// Read data in multiples of the page size of this pager.
    /// The data read if a concurrent write is happening is undefined.
    unsafe fn read_pages(&self, page: u64, count: usize, paddr: usize) -> EResult<()>;

    /// Write data in multiples of the page size of this pager.
    /// The data written if multiple concurrent writes happen is undefined.
    unsafe fn write_pages(&self, page: u64, count: usize, paddr: usize) -> EResult<()>;
}
