// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

use crate::{
    badgelib::irq::IrqGuard,
    bindings::{
        error::{EResult, Errno},
        raw::timestamp_us_t,
    },
    config::PAGE_SIZE,
    kernel::sync::{spinlock::Spinlock, waitlist::Waitlist},
    mem::pmm::{self, PAddrr},
    util::rtree::RadixTree,
};

mod flags {
    /// Entry contains differences from the backing store.
    pub const DIRTY: u32 = 1 << 0;
    /// Entry is being written to disk.
    pub const WRITING: u32 = 1 << 1;
    /// Entry is being read from disk.
    pub const READING: u32 = 1 << 2;
}

/// Metadata about a block/page in a [`PageCache`].
struct Entry {
    paddr: PAddrr,
    flags: AtomicU32,
}

/// Generic page cache implementation that translates between system page size and cached block size.
pub struct PageCache {
    /// Log-base 2 number of bytes a block is.
    block_size_exp: u8,
    /// Log-base 2 number of pages an entry is.
    entry_size_exp: u8,
    pages: Spinlock<RadixTree<u64, Entry>>,
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
    fn index(&self, offset: u64) -> (u64, usize) {
        assert!(offset % PAGE_SIZE as u64 == 0);
        let page = offset / PAGE_SIZE as u64;
        let page_size_exp = PAGE_SIZE.ilog2() as u8;
        if page_size_exp > self.block_size_exp {
            (1, 0)
        } else {
            let page_per_block = 1 << (self.block_size_exp - page_size_exp);
            (
                page * page_per_block,
                (page % page_per_block) as usize * PAGE_SIZE as usize,
            )
        }
    }

    /// Read data into an entry from disk.
    unsafe fn read_from_disk(&self, pager: &dyn Pager, index: u64, paddr: PAddrr) -> EResult<()> {
        todo!()
    }

    /// Write data from an entry to disk.
    unsafe fn write_to_disk(&self, pager: &dyn Pager, index: u64, paddr: PAddrr) -> EResult<()> {
        todo!()
    }

    /// Try to get an existing entry.
    fn get_existing(&self, addr: u64) -> EResult<Option<PAddrr>> {
        let (index, offset) = self.index(addr);

        let _noirq = IrqGuard::new();
        let mut pages = self.pages.lock_shared();
        let mut ent = match pages.get(index) {
            Some(x) => x,
            None => return Ok(None),
        };

        while (ent.flags.load(Ordering::Relaxed) & flags::READING) != 0 {
            drop(pages);

            self.read_waitlist.block(timestamp_us_t::MAX, || {
                self.pages
                    .lock_shared()
                    .get(index)
                    .map(|x| x.flags.load(Ordering::Relaxed) & flags::READING != 0)
                    .unwrap_or(false)
            })?;

            pages = self.pages.lock_shared();
            ent = match pages.get(index) {
                Some(x) => x,
                None => return Ok(None),
            };
        }

        // Page can now be mapped.
        unsafe {
            let meta = pmm::page_struct(ent.paddr);
            (*meta).refcount.fetch_add(1, Ordering::Relaxed);
        }

        Ok(Some(ent.paddr + offset))
    }

    /// Get a page from the cache and increase its refcount.
    pub fn get(&self, pager: &dyn Pager, addr: u64) -> EResult<PAddrr> {
        let (index, offset) = self.index(addr);

        loop {
            if let Some(x) = self.get_existing(addr)? {
                return Ok(x);
            }

            let mut pages = self.pages.lock();
            if pages.get(index).is_some() {
                // Another thread has already allocated this entry.
                drop(pages);
                continue;
            }

            // Allocate a new entry.
            unsafe {
                let paddr = pmm::page_alloc(self.entry_size_exp, pmm::PageUsage::Cache)?;

                // Insert new entry marked as being read in.
                let noirq = IrqGuard::new();
                let entry = Entry {
                    paddr,
                    flags: AtomicU32::new(flags::READING),
                };
                let res = pages.insert(index, entry);
                drop(pages);
                drop(noirq);
                if res.is_err() {
                    // Insert FAILED, free the memory.
                    pmm::page_free(paddr, self.entry_size_exp);
                    return Err(Errno::ENOMEM);
                }

                // Actually read in the data.
                if let Err(x) = self.read_from_disk(pager, index, paddr) {
                    // Read FAILED, remove the pending entry again.
                    self.pages.lock().remove(index);
                    pmm::page_free(paddr, self.entry_size_exp);
                    return Err(x);
                }

                // Read SUCCESS, mark entry as writable.
                let meta = pmm::page_struct(paddr);
                (*meta).refcount.fetch_add(1, Ordering::Relaxed);
                let noirq = IrqGuard::new();
                self.pages
                    .lock_shared()
                    .get(index)
                    .expect("PageCache pending read deleted by another thread")
                    .flags
                    .store(0, Ordering::Relaxed);
                drop(noirq);

                return Ok(paddr + offset);
            }
        }
    }

    /// Mark a page as being dirty.
    pub fn mark_dirty(&self, pager: &dyn Pager, addr: u64) {
        let (index, _) = self.index(addr);

        let pages = self.pages.lock_shared();
    }

    /// Synchronize a page to disk.
    pub fn sync(&self, pager: &dyn Pager, addr: u64, flush: bool) -> EResult<()> {
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
    unsafe fn read_pages(&self, offset: u64, count: usize, paddr: PAddrr) -> EResult<()>;

    /// Write data in multiples of the page size of this pager.
    /// The data written if multiple concurrent writes happen is undefined.
    unsafe fn write_pages(&self, offset: u64, count: usize, paddr: PAddrr) -> EResult<()>;
}
