// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#[cfg(feature = "ktest")]
use core::cell::RefCell;
use core::{
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering},
};

use alloc::vec::Vec;

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
    process::usercopy::{UserSlice, UserSliceMut},
    util::rtree::RadixTree,
};

use super::{
    HHDM_OFFSET,
    memobject::{MappablePage, MemObject},
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
    pages: Spinlock<RadixTree<u64, Entry>>,
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
        let entry_pages_exp = entry_size_exp - PAGE_SIZE.ilog2() as u8;
        let entry_blocks_exp = entry_size_exp - block_size_exp;
        Self {
            block_size_exp,
            entry_pages_exp,
            entry_blocks_exp,
            pages: Spinlock::new(RadixTree::new()),
            len: Mutex::new(len),
            read_waitlist: Waitlist::new(),
            write_waitlist: Waitlist::new(),
        }
    }

    /// Change the page cache's length.
    pub fn set_len(&self, new_len: u64) {
        let mut guard = self.len.unintr_lock();
        *guard = new_len;
        logkf!(LogLevel::Warning, "TODO: PageCache::set_len invalidation");
    }

    /// Index calculation helper: returns (entry_index, page_within_entry).
    ///
    /// For block_size < page_size: multiple blocks share one page; entry_index = offset / page_size.
    /// For block_size > page_size: multiple pages share one block; page_within_entry > 0 is possible.
    /// entry_size_exp = block_size_exp + entry_blocks_exp = max(block_size_exp, page_size_exp).
    #[inline(always)]
    fn index(&self, offset: u64) -> (u64, usize) {
        assert!(offset % PAGE_SIZE as u64 == 0);
        let entry_index = offset >> (self.block_size_exp + self.entry_blocks_exp);
        let page = ((offset >> PAGE_SIZE.ilog2()) & ((1u64 << self.entry_pages_exp) - 1)) as usize;
        (entry_index, page)
    }

    /// Read data into an entry from disk.
    unsafe fn read_from_disk(&self, pager: &dyn Pager, index: u64, paddr: PAddrr) -> EResult<()> {
        // Lock length while we're accessing.
        let byte_len = self.len.unintr_lock_shared();
        let block_len = (*byte_len).div_ceil(1 << self.block_size_exp);

        // Determine bounds.
        let start_block = index << self.entry_blocks_exp;
        let end_block = (start_block + (1 << self.entry_blocks_exp)).min(block_len);

        unsafe {
            let hhdm_slice = core::ptr::slice_from_raw_parts_mut(
                (paddr + HHDM_OFFSET) as *mut u8,
                ((end_block - start_block) as usize) << self.block_size_exp,
            );
            pager.read_blocks(
                start_block,
                (end_block - start_block) as usize,
                paddr,
                &mut *hhdm_slice,
            )?;
        }

        // Backfill with zeroes past the file's end — only on the last entry.
        let entry_end_block = start_block + (1u64 << self.entry_blocks_exp);
        if block_len <= entry_end_block {
            let entry_bytes = (PAGE_SIZE as usize) << self.entry_pages_exp;
            let valid_bytes = (*byte_len as usize)
                .saturating_sub(start_block as usize * (1usize << self.block_size_exp))
                .min(entry_bytes);
            if valid_bytes < entry_bytes {
                // SAFETY: `paddr` is valid for `2^entry_pages_exp` pages; valid_bytes < entry_bytes.
                unsafe {
                    core::ptr::write_bytes(
                        (paddr + valid_bytes + HHDM_OFFSET) as *mut u8,
                        0,
                        entry_bytes - valid_bytes,
                    );
                }
            }
        }

        Ok(())
    }

    /// Write data from an entry to disk.
    unsafe fn write_to_disk(&self, pager: &dyn Pager, index: u64, paddr: PAddrr) -> EResult<()> {
        // Lock length while we're accessing.
        let byte_len = self.len.unintr_lock_shared();
        let block_len = (*byte_len).div_ceil(1 << self.block_size_exp);

        // Determine bounds.
        let start_block = index << self.entry_blocks_exp;
        let end_block = (start_block + (1 << self.entry_blocks_exp)).min(block_len);

        unsafe {
            let hhdm_slice = core::ptr::slice_from_raw_parts(
                (paddr + HHDM_OFFSET) as *const u8,
                ((end_block - start_block) as usize) << self.block_size_exp,
            );
            pager.write_blocks(
                start_block,
                (end_block - start_block) as usize,
                paddr,
                &*hhdm_slice,
            )
        }
    }

    /// Try to get an existing entry.
    pub fn get(&self, addr: u64) -> EResult<Option<MappablePage>> {
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

        // SAFETY: We own this physical address through the cache entries.
        Ok(Some(unsafe {
            MappablePage::new(ent.paddr + offset * PAGE_SIZE as usize, true, true, true)
        }))
    }

    /// Get or allocate a page from the cache and increase its refcount.
    pub fn alloc(&self, pager: &dyn Pager, addr: u64) -> EResult<MappablePage> {
        let (index, offset) = self.index(addr);
        let memobject = pager.memobject();

        loop {
            if let Some(x) = self.get(addr)? {
                return Ok(x);
            }

            let mut inner = self.pages.lock();
            if inner.get(index).is_some() {
                // Another thread has already allocated this entry.
                drop(inner);
                continue;
            }

            // Allocate a new entry.
            unsafe {
                let paddr = pmm::page_alloc(self.entry_pages_exp, pmm::PageUsage::Cache)?;
                let meta = pmm::page_struct(paddr);
                (*meta).memobject.replace(memobject);
                (*meta)
                    .offset
                    .replace(index << (self.entry_pages_exp + PAGE_SIZE.ilog2() as u8));

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
                    self.pages.lock().remove(index);
                    pmm::page_free(paddr, self.entry_pages_exp);
                    return Err(x);
                }

                // Read SUCCESS, mark entry as writable.
                (*meta).refcount.fetch_add(1, Ordering::Relaxed);
                let noirq = IrqGuard::new();
                self.pages
                    .lock_shared()
                    .get(index)
                    .expect("PageCache pending read deleted by another thread")
                    .flags
                    .store(0, Ordering::Relaxed);
                drop(noirq);

                // SAFETY: We own this physical address through the cache entries.
                return Ok(MappablePage::new(
                    paddr + offset * PAGE_SIZE as usize,
                    true,
                    true,
                    true,
                ));
            }
        }
    }

    /// Returns true if any cached entry is currently dirty.
    pub fn has_dirty_pages(&self) -> bool {
        let pages = self.pages.lock_shared();
        pages
            .iter()
            .any(|(_, e)| e.flags.load(Ordering::Relaxed) & flags::DIRTY != 0)
    }

    /// Mark a page as being dirty.
    pub fn mark_dirty(&self, addr: u64) {
        let (index, _) = self.index(addr);

        let inner = self.pages.lock_shared();
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

    /// Common implementation of [`Self::sync`] and [`Self::sync_all`].
    fn sync_impl(&self, pager: &dyn Pager, to_sync: &[u64]) -> EResult<()> {
        for &index in to_sync {
            let pages = self.pages.lock_shared();
            let entry = match pages.get(index) {
                Some(x) => x,
                None => continue,
            };
            let paddr = entry.paddr;

            // Filter for dirty entries not being written back already.
            if entry
                .flags
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |x| {
                    ((x & (flags::DIRTY | flags::WRITING)) == flags::DIRTY).then_some(0)
                })
                .is_err()
            {
                continue;
            }

            // Attempt to write back to disk.
            drop(pages);
            let res = unsafe { self.write_to_disk(pager, index, paddr) };
            let pages = self.pages.lock_shared();
            let entry = pages
                .get(index)
                .expect("PageCache pending write deleted by another thread");

            if res.is_err() {
                // Failed to write; re-mark dirty.
                entry.flags.store(flags::DIRTY, Ordering::Relaxed);
                // We'd rather silently succeed part of a sync returning Err than silently fail part of a sync returning Ok.
                return res;
            } else {
                // This write succeeded.
                entry.flags.store(0, Ordering::Relaxed);
            }
        }

        // All writes succeeded.
        Ok(())
    }

    /// Synchronize a page to disk.
    pub fn sync(&self, pager: &dyn Pager, addr: u64, len: u64) -> EResult<()> {
        // Lock size while finding which pages to sync.
        let byte_len = self.len.unintr_lock_shared();
        let start_index = self.index(addr).0;
        let end_index = self.index((addr + len.saturating_sub(1)).min(*byte_len)).0;

        // Collect a list of entries to sync.
        let mut to_sync = Vec::new();
        let pages = self.pages.lock_shared();
        for index in start_index..(end_index + 1) {
            if let Some(entry) = pages.get(index) {
                if entry.flags.load(Ordering::Relaxed) & flags::DIRTY != 0 {
                    to_sync.try_reserve(1)?;
                    to_sync.push(index);
                }
            }
        }
        drop(pages);
        drop(byte_len);

        self.sync_impl(pager, &to_sync)
    }

    /// Synchronize all pages to disk.
    pub fn sync_all(&self, pager: &dyn Pager) -> EResult<()> {
        // Lock size while finding which pages to sync.
        let byte_len = self.len.unintr_lock_shared();

        // Collect a list of entries to sync.
        let mut to_sync = Vec::new();
        let pages = self.pages.lock_shared();
        for (index, entry) in pages.iter() {
            if entry.flags.load(Ordering::Relaxed) & flags::DIRTY != 0 {
                to_sync.try_reserve(1)?;
                to_sync.push(index);
            }
        }
        drop(pages);
        drop(byte_len);

        self.sync_impl(pager, &to_sync)
    }

    /// Flush clean, unreferenced cache entries.
    pub fn flush(&self) {
        // Collect eviction candidates and remove them from the tree under the exclusive
        // lock.  Holding exclusive prevents get_existing from incrementing a refcount
        // between our check and the removal, which would free a live page.
        let mut to_free: Vec<(PAddrr, u8)> = Vec::new();
        {
            let _noirq = IrqGuard::new();
            let mut pages = self.pages.lock();

            let mut to_remove = Vec::new();
            for (index, entry) in pages.iter() {
                if entry.flags.load(Ordering::Relaxed)
                    & (flags::DIRTY | flags::WRITING | flags::READING)
                    != 0
                {
                    continue;
                }
                // SAFETY: paddr is valid for the lifetime of the cache entry.
                let refcount = unsafe {
                    (*pmm::page_struct(entry.paddr))
                        .refcount
                        .load(Ordering::Relaxed)
                };
                if refcount == 0 {
                    // Best-effort: skip on OOM rather than panic inside a lock.
                    if to_remove.try_reserve(1).is_err() || to_free.try_reserve(1).is_err() {
                        break;
                    }
                    to_remove.push(index);
                    to_free.push((entry.paddr, self.entry_pages_exp));
                }
            }

            for index in to_remove {
                pages.remove(index);
            }
        }

        for (paddr, order) in to_free {
            // SAFETY: entry has been removed from the cache with refcount confirmed 0.
            unsafe { pmm::page_free(paddr, order) };
        }
    }

    /// Read bytes through the cache.
    pub fn read_bytes(
        &self,
        pager: &dyn Pager,
        addr: u64,
        mut rdata: UserSliceMut<'_, u8>,
    ) -> EResult<()> {
        let len = rdata.len() as u64;
        let mut progress: u64 = 0;
        while progress < len {
            let cur_addr = addr + progress;
            let page_base = cur_addr & !(PAGE_SIZE as u64 - 1);
            let page_offset = (cur_addr - page_base) as usize;
            let copy_len = (PAGE_SIZE as usize - page_offset).min((len - progress) as usize);

            let page = self.alloc(pager, page_base)?;
            let src_vaddr = page.paddr() + unsafe { HHDM_OFFSET } + page_offset;
            // SAFETY: paddr from the cache is valid for PAGE_SIZE bytes; copy_len fits within it.
            let src_slice =
                unsafe { core::slice::from_raw_parts(src_vaddr as *const u8, copy_len) };
            rdata.write_multiple(progress as usize, src_slice)?;

            progress += copy_len as u64;
        }
        Ok(())
    }

    /// Write bytes through the cache.
    pub fn write_bytes(
        &self,
        pager: &dyn Pager,
        addr: u64,
        wdata: UserSlice<'_, u8>,
    ) -> EResult<()> {
        let len = wdata.len() as u64;
        let mut progress: u64 = 0;
        while progress < len {
            let cur_addr = addr + progress;
            let page_base = cur_addr & !(PAGE_SIZE as u64 - 1);
            let page_offset = (cur_addr - page_base) as usize;
            let copy_len = (PAGE_SIZE as usize - page_offset).min((len - progress) as usize);

            let page = self.alloc(pager, page_base)?;
            let dst_vaddr = page.paddr() + unsafe { HHDM_OFFSET } + page_offset;
            // SAFETY: paddr from the cache is valid for PAGE_SIZE bytes; copy_len fits within it.
            let dst_slice =
                unsafe { core::slice::from_raw_parts_mut(dst_vaddr as *mut u8, copy_len) };
            wdata.read_multiple(progress as usize, dst_slice)?;
            // Mark dirty before dropping page so flush can't evict the just-written entry.
            self.mark_dirty(page_base);

            progress += copy_len as u64;
        }
        Ok(())
    }
}

impl Drop for PageCache {
    fn drop(&mut self) {
        let mut refs = 0;
        let mut dirty = 0;

        for entry in self.pages.lock().iter() {
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
    /// Memory object to associate with pages, if any.
    fn memobject(&self) -> Option<NonNull<dyn MemObject>>;

    /// Read data in multiples of the block size of this pager.
    /// The data read if a concurrent write is happening is undefined.
    unsafe fn read_blocks(
        &self,
        start_block: u64,
        block_count: usize,
        paddr: PAddrr,
        vaddr: &mut [u8],
    ) -> EResult<()>;

    /// Write data in multiples of the block size of this pager.
    /// The data written if multiple concurrent writes happen is undefined.
    unsafe fn write_blocks(
        &self,
        start_block: u64,
        block_count: usize,
        paddr: PAddrr,
        vaddr: &[u8],
    ) -> EResult<()>;
}

#[cfg(feature = "ktest")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestPagerOp {
    Read {
        start_block: u64,
        block_count: usize,
    },
    Write {
        start_block: u64,
        block_count: usize,
    },
}

#[cfg(feature = "ktest")]
struct TestPager {
    block_size_exp: u8,
    data: RefCell<Vec<u8>>,
    log: RefCell<Vec<TestPagerOp>>,
}

#[cfg(feature = "ktest")]
impl TestPager {
    fn new(block_size_exp: u8, size: usize) -> EResult<Self> {
        let mut data = Vec::try_with_capacity(size)?;
        data.resize(size, 0);
        Ok(Self {
            block_size_exp,
            data: RefCell::new(data),
            log: RefCell::new(Vec::new()),
        })
    }

    fn size(&self) -> u64 {
        self.data.borrow().len() as u64
    }

    fn reads(&self) -> usize {
        self.log
            .borrow()
            .iter()
            .filter(|op| matches!(op, TestPagerOp::Read { .. }))
            .count()
    }

    fn writes(&self) -> usize {
        self.log
            .borrow()
            .iter()
            .filter(|op| matches!(op, TestPagerOp::Write { .. }))
            .count()
    }
}

#[cfg(feature = "ktest")]
impl Pager for TestPager {
    fn memobject(&self) -> Option<NonNull<dyn MemObject>> {
        None
    }

    unsafe fn read_blocks(
        &self,
        start_block: u64,
        block_count: usize,
        _paddr: PAddrr,
        vaddr: &mut [u8],
    ) -> EResult<()> {
        let op = TestPagerOp::Read {
            start_block,
            block_count,
        };
        self.log.borrow_mut().push(op);

        let block_size = 1usize << self.block_size_exp;
        let src_start = start_block as usize * block_size;
        let data = self.data.borrow();
        let src = &data[src_start..src_start + block_count * block_size];
        vaddr.copy_from_slice(src);
        Ok(())
    }

    unsafe fn write_blocks(
        &self,
        start_block: u64,
        block_count: usize,
        _paddr: PAddrr,
        vaddr: &[u8],
    ) -> EResult<()> {
        let op = TestPagerOp::Write {
            start_block,
            block_count,
        };
        self.log.borrow_mut().push(op);

        let block_size = 1usize << self.block_size_exp;
        let dst_start = start_block as usize * block_size;
        let mut data = self.data.borrow_mut();
        data[dst_start..dst_start + block_count * block_size].copy_from_slice(vaddr);
        Ok(())
    }
}

vmm_ktest! { PAGECACHE_READ,
    // Verify that data from the backing store is correctly read into the cache.
    let pager = TestPager::new(PAGE_SIZE.ilog2() as u8, PAGE_SIZE as usize * 4)?;
    pager.data.borrow_mut()[0] = 0xAB;
    pager.data.borrow_mut()[PAGE_SIZE as usize] = 0xCD;
    let cache = PageCache::new(pager.block_size_exp, pager.size());

    let page0 = cache.alloc(&pager, 0)?;
    let val0 = unsafe { *((page0.paddr() + HHDM_OFFSET) as *const u8) };
    ktest_expect!(val0, 0xABu8);

    let page1 = cache.alloc(&pager, PAGE_SIZE as u64)?;
    let val1 = unsafe { *((page1.paddr() + HHDM_OFFSET) as *const u8) };
    ktest_expect!(val1, 0xCDu8);
}

vmm_ktest! { PAGECACHE_DIRTY_WRITEBACK,
    // Verify that a dirty page is flushed to the backing store on sync.
    let pager = TestPager::new(PAGE_SIZE.ilog2() as u8, PAGE_SIZE as usize * 4)?;
    let cache = PageCache::new(pager.block_size_exp, pager.size());

    let page = cache.alloc(&pager, 0)?;
    unsafe { *((page.paddr() + HHDM_OFFSET) as *mut u8) = 0xBE; }
    cache.mark_dirty(0);

    cache.sync_all(&pager)?;

    ktest_expect!(pager.data.borrow()[0], 0xBEu8);
}

vmm_ktest! { PAGECACHE_NO_REDUNDANT_READ,
    // Verify that a cached page is not re-read from disk when already present.
    let pager = TestPager::new(PAGE_SIZE.ilog2() as u8, PAGE_SIZE as usize * 4)?;
    let cache = PageCache::new(pager.block_size_exp, pager.size());

    let page = cache.alloc(&pager, 0)?;
    drop(page);
    ktest_expect!(pager.reads(), 1usize);

    let page = cache.alloc(&pager, 0)?;
    drop(page);
    ktest_expect!(pager.reads(), 1usize);
}

vmm_ktest! { PAGECACHE_NO_REDUNDANT_WRITE,
    // Verify that a clean page is not written to disk on a subsequent sync.
    let pager = TestPager::new(PAGE_SIZE.ilog2() as u8, PAGE_SIZE as usize * 4)?;
    let cache = PageCache::new(pager.block_size_exp, pager.size());

    let page = cache.alloc(&pager, 0)?;
    unsafe { *((page.paddr() + HHDM_OFFSET) as *mut u8) = 0xEF; }
    cache.mark_dirty(0);
    cache.sync_all(&pager)?;
    ktest_expect!(pager.writes(), 1usize);

    // Page is now clean; a second sync must not write again.
    cache.sync_all(&pager)?;
    ktest_expect!(pager.writes(), 1usize);
}

vmm_ktest! { PAGECACHE_FLUSH_EVICTS_CLEAN,
    // A clean, unreferenced page must be evicted by flush and re-read on next access.
    let pager = TestPager::new(PAGE_SIZE.ilog2() as u8, PAGE_SIZE as usize * 4)?;
    pager.data.borrow_mut()[0] = 0x11;
    let cache = PageCache::new(pager.block_size_exp, pager.size());

    let page = cache.alloc(&pager, 0)?;
    drop(page);  // refcount -> 0, still clean
    ktest_expect!(pager.reads(), 1usize);

    cache.flush();

    // Entry is gone; next get must trigger a fresh read.
    let page = cache.alloc(&pager, 0)?;
    ktest_expect!(pager.reads(), 2usize);
    let val = unsafe { *((page.paddr() + HHDM_OFFSET) as *const u8) };
    ktest_expect!(val, 0x11u8);
}

vmm_ktest! { PAGECACHE_FLUSH_RETAINS_DIRTY,
    // A dirty, unreferenced page must survive flush to avoid data loss.
    let pager = TestPager::new(PAGE_SIZE.ilog2() as u8, PAGE_SIZE as usize * 4)?;
    let cache = PageCache::new(pager.block_size_exp, pager.size());

    let page = cache.alloc(&pager, 0)?;
    unsafe { *((page.paddr() + HHDM_OFFSET) as *mut u8) = 0x22; }
    cache.mark_dirty(0);
    drop(page);  // refcount -> 0, but dirty

    cache.flush();

    // Page must still be in cache with its dirty data intact; no new read.
    let page = cache.alloc(&pager, 0)?;
    ktest_expect!(pager.reads(), 1usize);
    let val = unsafe { *((page.paddr() + HHDM_OFFSET) as *const u8) };
    ktest_expect!(val, 0x22u8);

    // Now sync so that the PageCache doesn't print the dirty warning.
    cache.sync_all(&pager)?;
}

vmm_ktest! { PAGECACHE_FLUSH_RETAINS_REFERENCED,
    // A page with an outstanding MappablePage (refcount > 0) must not be evicted.
    let pager = TestPager::new(PAGE_SIZE.ilog2() as u8, PAGE_SIZE as usize * 4)?;
    pager.data.borrow_mut()[0] = 0x33;
    let cache = PageCache::new(pager.block_size_exp, pager.size());

    let page = cache.alloc(&pager, 0)?;  // refcount = 1
    cache.flush();  // must not evict; refcount != 0

    // Page is still accessible with no additional disk read.
    ktest_expect!(pager.reads(), 1usize);
    let val = unsafe { *((page.paddr() + HHDM_OFFSET) as *const u8) };
    ktest_expect!(val, 0x33u8);
}

vmm_ktest! { PAGECACHE_SMALL_BLOCK_CROSS_ENTRY_READ,
    // block_size (512) < page_size (4096): 8 blocks per entry, each entry is exactly 1 page.
    // Page 0 lives in entry 0 (blocks 0-7), page 1 lives in entry 1 (blocks 8-15).
    let pager = TestPager::new(9, PAGE_SIZE as usize * 2)?;
    pager.data.borrow_mut()[0] = 0xAA;
    pager.data.borrow_mut()[PAGE_SIZE as usize] = 0xBB;
    let cache = PageCache::new(9, pager.size());

    let page0 = cache.alloc(&pager, 0)?;
    let val0 = unsafe { *((page0.paddr() + HHDM_OFFSET) as *const u8) };
    ktest_expect!(val0, 0xAAu8);

    // A second entry must be loaded from disk for the second page.
    let page1 = cache.alloc(&pager, PAGE_SIZE as u64)?;
    let val1 = unsafe { *((page1.paddr() + HHDM_OFFSET) as *const u8) };
    ktest_expect!(val1, 0xBBu8);

    ktest_expect!(pager.reads(), 2usize);
}

vmm_ktest! { PAGECACHE_LARGE_BLOCK_INTRA_ENTRY_PAGES,
    // block_size (8192) > page_size (4096): 1 block spans 2 pages, both in the same entry.
    // Both pages must be satisfied by a single disk read.
    let block_size_exp = PAGE_SIZE.ilog2() as u8 + 1;
    let pager = TestPager::new(block_size_exp, PAGE_SIZE as usize * 2)?;
    pager.data.borrow_mut()[0] = 0xCC;
    pager.data.borrow_mut()[PAGE_SIZE as usize] = 0xDD;
    let cache = PageCache::new(block_size_exp, pager.size());

    let page0 = cache.alloc(&pager, 0)?;
    let val0 = unsafe { *((page0.paddr() + HHDM_OFFSET) as *const u8) };
    ktest_expect!(val0, 0xCCu8);

    // Second page is in the same cache entry; must not trigger another disk read.
    let page1 = cache.alloc(&pager, PAGE_SIZE as u64)?;
    let val1 = unsafe { *((page1.paddr() + HHDM_OFFSET) as *const u8) };
    ktest_expect!(val1, 0xDDu8);

    ktest_expect!(pager.reads(), 1usize);
}

vmm_ktest! { PAGECACHE_SMALL_BLOCK_DIRTY_WRITEBACK_SECOND_ENTRY,
    // block_size (512) < page_size (4096): dirty data in entry 1 must be written to the
    // correct block range (blocks 8-15 = bytes 4096-8191), not entry 0's range.
    let pager = TestPager::new(9, PAGE_SIZE as usize * 2)?;
    let cache = PageCache::new(9, pager.size());

    let page1 = cache.alloc(&pager, PAGE_SIZE as u64)?;
    unsafe { *((page1.paddr() + HHDM_OFFSET) as *mut u8) = 0xBE; }
    cache.mark_dirty(PAGE_SIZE as u64);

    cache.sync_all(&pager)?;

    ktest_expect!(pager.data.borrow()[PAGE_SIZE as usize], 0xBEu8);
    // Only entry 1 was dirty; exactly one writeback must have occurred.
    ktest_expect!(pager.writes(), 1usize);
}
