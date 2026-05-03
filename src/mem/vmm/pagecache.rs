// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

use crate::{
    badgelib::irq::IrqGuard,
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
        raw::timestamp_us_t,
    },
    config::PAGE_SIZE,
    kernel::sync::{mutex::Mutex, spinlock::Spinlock, waitlist::Waitlist},
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
    entry_pages_exp: u8,
    /// Log-base 2 number of blocks per entry.
    entry_blocks_exp: u8,
    /// Cached page metadata.
    inner: Spinlock<RadixTree<u64, Entry>>,
    /// Total object size in bytes.
    len: Mutex<u64>,
    /// Threads waiting for disk reads.
    read_waitlist: Waitlist,
    /// Threads waiting for disk writes.
    write_waitlist: Waitlist,
}

impl PageCache {
    pub fn new(block_size_exp: u8, len: u64) -> Self {
        let entry_size_exp = block_size_exp.max(PAGE_SIZE.ilog2() as u8);
        let entry_pages_exp = block_size_exp - PAGE_SIZE.ilog2() as u8;
        let entry_blocks_exp = entry_size_exp - block_size_exp;
        Self {
            block_size_exp,
            entry_pages_exp,
            entry_blocks_exp,
            inner: Spinlock::new(RadixTree::new()),
            len: Mutex::new(len),
            read_waitlist: Waitlist::new(),
            write_waitlist: Waitlist::new(),
        }
    }

    /// Index calculation helper.
    #[inline(always)]
    fn index(&self, offset: u64) -> (u64, usize) {
        assert!(offset % PAGE_SIZE as u64 == 0);
        let block = offset >> self.block_size_exp;
        let page = offset % (1 << self.block_size_exp) / PAGE_SIZE as u64;
        (block, page as usize)
    }

    /// Read data into an entry from disk.
    unsafe fn read_from_disk(&self, pager: &dyn Pager, index: u64, paddr: PAddrr) -> EResult<()> {
        // Lock length while we're accessing.
        let len = self.len.unintr_lock_shared();

        // Determine bounds.

        // Backfill the remaining bytes with zeroes.

        Ok(())
    }

    /// Write data from an entry to disk.
    unsafe fn write_to_disk(&self, pager: &dyn Pager, index: u64, paddr: PAddrr) -> EResult<()> {
        // Lock length while we're accessing.
        let len = self.len.unintr_lock_shared();

        // Determine bounds.

        Ok(())
    }

    /// Try to get an existing entry.
    fn get_existing(&self, addr: u64) -> EResult<Option<PAddrr>> {
        let (index, offset) = self.index(addr);

        let _noirq = IrqGuard::new();
        let mut pages = self.inner.lock_shared();
        let mut ent = match pages.get(index) {
            Some(x) => x,
            None => return Ok(None),
        };

        while (ent.flags.load(Ordering::Relaxed) & flags::READING) != 0 {
            drop(pages);

            self.read_waitlist.block(timestamp_us_t::MAX, || {
                self.inner
                    .lock_shared()
                    .get(index)
                    .map(|x| x.flags.load(Ordering::Relaxed) & flags::READING != 0)
                    .unwrap_or(false)
            })?;

            pages = self.inner.lock_shared();
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

            let mut inner = self.inner.lock();
            if inner.get(index).is_some() {
                // Another thread has already allocated this entry.
                drop(inner);
                continue;
            }

            // Allocate a new entry.
            unsafe {
                let paddr = pmm::page_alloc(self.entry_pages_exp, pmm::PageUsage::Cache)?;

                // Insert new entry marked as being read in.
                let noirq = IrqGuard::new();
                let entry = Entry {
                    paddr,
                    flags: AtomicU32::new(flags::READING),
                };
                let res = inner.insert(index, entry);
                drop(inner);
                drop(noirq);
                if res.is_err() {
                    // Insert FAILED, free the memory.
                    pmm::page_free(paddr, self.entry_pages_exp);
                    return Err(Errno::ENOMEM);
                }

                // Actually read in the data.
                if let Err(x) = self.read_from_disk(pager, index, paddr) {
                    // Read FAILED, remove the pending entry again.
                    self.inner.lock().remove(index);
                    pmm::page_free(paddr, self.entry_pages_exp);
                    return Err(x);
                }

                // Read SUCCESS, mark entry as writable.
                let meta = pmm::page_struct(paddr);
                (*meta).refcount.fetch_add(1, Ordering::Relaxed);
                let noirq = IrqGuard::new();
                self.inner
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
    pub fn mark_dirty(&self, _pager: &dyn Pager, addr: u64) {
        let (index, _) = self.index(addr);

        let inner = self.inner.lock_shared();
        if let Some(entry) = inner.get(index) {
            let prev = entry.flags.fetch_or(flags::DIRTY, Ordering::Relaxed);
            if prev & flags::READING != 0 {
                // This is a bug.
                logkf!(
                    LogLevel::Warning,
                    "PageCache: Entry marked dirty before read is finished"
                );
            }
        }
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

impl Drop for PageCache {
    fn drop(&mut self) {
        let mut refs = 0;
        let mut dirty = 0;

        for entry in self.inner.lock().iter() {
            if entry.1.flags.load(Ordering::Relaxed) & flags::DIRTY != 0 {
                dirty += 1;
            }
            unsafe {
                let meta = pmm::page_struct(entry.1.paddr);
                if (*meta).refcount.load(Ordering::Relaxed) == 0 {
                    pmm::page_free(entry.1.paddr, self.entry_pages_exp);
                } else {
                    refs += 1;
                }
            }
        }

        // Either of these variables being nonzero is a bug.
        if refs > 0 {
            logkf!(
                LogLevel::Error,
                "PageCache: {} blocks have stale references (memory leaked and likely data loss)",
                refs
            );
        }
        if dirty > 0 {
            logkf!(
                LogLevel::Warning,
                "PageCache: {} blocks are still dirty (likely data loss)",
                dirty
            );
        }
    }
}

/// Interface for reading and writing blocks for a [`PageCache`] (note: not locked to system page size).
pub trait Pager {
    /// Log-base 2 number of bytes a block is.
    fn block_size_exp(&self) -> u8;

    /// Read data in multiples of the block size of this blockr.
    /// The data read if a concurrent write is happening is undefined.
    unsafe fn read_blocks(&self, offset: u64, size: usize, paddr: PAddrr) -> EResult<()>;

    /// Write data in multiples of the block size of this blockr.
    /// The data written if multiple concurrent writes happen is undefined.
    unsafe fn write_blocks(&self, offset: u64, size: usize, paddr: PAddrr) -> EResult<()>;
}
