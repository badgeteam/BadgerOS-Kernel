// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ops::Range;

use alloc::{boxed::Box, sync::Arc, vec::Vec};

use crate::{
    bindings::error::EResult,
    config::PAGE_SIZE,
    impl_has_list_node,
    kernel::sync::spinlock::Spinlock,
    mem::{
        pmm::{self, PPN},
        vmm::physmap::PhysMap,
    },
    util::list::{InvasiveList, InvasiveListNode},
};

use super::*;

/// Mapping is shared (not CoW'ed on fork).
pub const SHARED: u8 = 0x01;
/// Mapping is private (CoW'ed on fork).
pub const PRIVATE: u8 = 0x02;
/// Replace the mapping at the given address if it exists.
pub const FIXED: u8 = 0x10;

/// A region of anonymous memory.
struct Anon {
    ppn: PPN,
}

#[derive(Clone)]
struct AnonMap {
    /// Offset from the parent [`MapEntry`].
    offset: VPN,
    /// Resident pages of this region.
    pages: Vec<Option<Arc<Anon>>>,
}

impl AnonMap {
    /// Determine whether this amap would need trimming to fit in the given range.
    pub fn needs_trim(&self, subrange: Range<VPN>) -> bool {
        subrange.start > self.offset || subrange.end < self.offset + self.pages.len()
    }

    /// Trim pages from the start and/or end of this amap.
    pub fn trim(&self, subrange: Range<VPN>) -> EResult<Option<Self>> {
        if subrange.end <= self.offset || subrange.start >= self.offset + self.pages.len() {
            return Ok(None);
        }

        let offset = self.offset.max(subrange.start);
        let mut pages =
            Vec::try_with_capacity((self.offset + self.pages.len()).min(subrange.end) - offset)?;
        pages.extend_from_slice(
            &self.pages[offset - self.offset..offset - self.offset + pages.len()],
        );

        Ok(Some(Self { offset, pages }))
    }

    /// Split this entry into two starting at `offset`.
    pub fn split(&self, offset: VPN) -> EResult<(Option<Self>, Option<Self>)> {
        if offset < self.offset {
            return Ok((None, Some(self.clone())));
        }
        if offset == self.offset {
            return Ok((Some(self.clone()), None));
        }

        let mut first = Vec::try_with_capacity(offset - self.offset)?;
        let mut second = Vec::try_with_capacity(self.pages.len() - first.len())?;
        first.extend_from_slice(&self.pages[..first.len()]);
        second.extend_from_slice(&self.pages[first.len()..]);

        Ok((
            Some(Self {
                offset: self.offset,
                pages: first,
            }),
            Some(Self {
                offset,
                pages: second,
            }),
        ))
    }

    /// Get the page currently mapped at `offset`.
    pub fn get_page(&self, offset: VPN) -> Option<PPN> {
        let offset = offset.checked_sub(self.offset)?;
        if offset > self.pages.len() {
            return None;
        }
        self.pages[(offset - self.offset) as usize]
            .as_deref()
            .map(|x| x.ppn)
    }

    /// Get or allocate the page at `offset`.
    /// If a new page is allocated, the existing data in the page at `orig` is copied.
    pub unsafe fn alloc_page(&mut self, offset: VPN, orig: Option<PPN>) -> EResult<PPN> {
        if let Some(page) = self.get_page(offset) {
            return Ok(page);
        }

        if self.pages.len() == 0 {
            self.offset = offset;
            self.pages.try_reserve(1)?;
            self.pages.push(None);
        } else if offset < self.offset {
            let shift = self.offset - offset;
            self.pages.try_reserve(shift)?;
            self.pages.splice(0..0, (0..shift).map(|_| None));
            self.offset -= shift;
        }

        let new_page;
        let anon;
        unsafe {
            new_page = pmm::page_alloc(0, pmm::PageUsage::UserAnon)?;
            anon = match Arc::try_new(Anon { ppn: new_page }) {
                Ok(x) => x,
                Err(x) => {
                    pmm::page_free(new_page);
                    return Err(x.into());
                }
            };

            let new_hhdm = new_page * PAGE_SIZE as usize + HHDM_OFFSET;
            if let Some(orig) = orig {
                let old_hhdm = orig * PAGE_SIZE as usize + HHDM_OFFSET;
                core::ptr::copy_nonoverlapping(
                    old_hhdm as *const u8,
                    new_hhdm as *mut u8,
                    PAGE_SIZE as usize,
                );
            } else {
                core::ptr::write_bytes(new_hhdm as *mut u8, 0, PAGE_SIZE as usize);
            }
        }

        self.pages[(offset - self.offset) as usize] = Some(anon);

        Ok(new_page)
    }
}

/// One contiguous range of mapped memory with the same protection and mapping flags.
struct MapEntry {
    node: InvasiveListNode,
    inner: Spinlock<MapEntryInner>,
}
impl_has_list_node!(MapEntry, node);

/// One contiguous range of mapped memory with the same protection and mapping flags.
struct MapEntryInner {
    /// Region start and end.
    range: Range<VPN>,
    /// Region protection flags.
    prot: u8,
    /// Region mapping flags.
    map: u8,
    /// Anonymous memory overlay.
    amap: Option<Arc<AnonMap>>,
    // TODO: Memory object offset and reference.
}

/// # Memory Leaks
/// This struct itself does not have all information needed to release pages to the underlying MemObject;
/// when the pmap is dropped, it must notify the MapEntry of all pages that were previously mapped.
impl MapEntryInner {
    /// Trim pages from the start and/or end of this entry.
    pub fn trim(&mut self, subrange: Range<VPN>) -> EResult<()> {
        debug_assert!(subrange.start >= self.range.start);
        debug_assert!(subrange.end <= self.range.end);

        if let Some(amap) = &mut self.amap
            && amap.needs_trim(subrange.clone())
        {
            if let Some(amap) = amap.trim(subrange.clone())? {
                self.amap = Some(Arc::try_new(amap)?);
            } else {
                self.amap = None;
            }
        }

        self.range = subrange;

        Ok(())
    }

    /// Split this entry into two starting at `offset`.
    /// On error, returns the old entry unchanged.
    pub fn split(&self, offset: VPN) -> EResult<(Self, Self)> {
        let mut first = Self {
            range: self.range.start..offset,
            prot: self.prot,
            map: self.map,
            amap: None,
        };
        let mut second = Self {
            range: self.range.start..offset,
            prot: self.prot,
            map: self.map,
            amap: None,
        };

        if let Some(amap) = self.amap.as_deref() {
            let (amap0, amap1) = amap.split(offset)?;
            if let Some(amap0) = amap0 {
                first.amap = Some(Arc::try_new(amap0)?);
            }
            if let Some(amap1) = amap1 {
                second.amap = Some(Arc::try_new(amap1)?);
            }
        }

        Ok((first, second))
    }

    /// Get the page currently mapped at `offset`.
    /// Must only be called if there is no page currently mapped for `offset`.
    pub fn get_page(&self, offset: VPN) -> Option<PPN> {
        if let Some(amap) = &self.amap
            && let Some(page) = amap.get_page(offset)
        {
            return Some(page);
        }

        // TODO: Fetch pages from MemObject, which could return None here.
        Some(unsafe { PAGE_OF_ZEROES })
    }

    /// Get or allocate the page at `offset`.
    /// If a backing page was already mapped in the pmap, the existing data at `orig` cay be copied into the anon.
    /// If an anon was created, the page `orig` is put back to the pager (its refcount decreased).
    pub unsafe fn alloc_page(
        &mut self,
        offset: VPN,
        mut for_writing: bool,
        orig: Option<PPN>,
    ) -> EResult<PPN> {
        if self.map & SHARED != 0 {
            // Shared memory will not create anons overlaying the backing memory.
            for_writing = false;
        }
        if !for_writing
            && let Some(amap) = &self.amap
            && let Some(page) = amap.get_page(offset)
        {
            return Ok(page);
        }

        if orig.is_none() {
            // TODO: Alloc pages from MemObject.
            return Ok(unsafe { PAGE_OF_ZEROES });
        }

        // Ensure we have a mutable reference to an AnonMap.
        if let Some(amap) = &mut self.amap {
            if !Arc::is_unique(amap) {
                // Duplicating the AnonMap here so private changes are copied on write.
                *amap = Arc::try_new((**amap).clone())?;
            }
        } else {
            self.amap = Some(Arc::try_new(AnonMap {
                offset: 0,
                pages: Vec::new(),
            })?);
        }
        let amap = Arc::get_mut(self.amap.as_mut().unwrap()).unwrap();

        unsafe {
            // TODO: Pass Some(orig) iff a MemObject is associated.
            let new = amap.alloc_page(offset, None)?;
            // TODO: Return orig to MemObject.

            Ok(new)
        }
    }

    /// Notify that a page has been dropped from the pmap for any reason.
    /// Used mostly for error handling cleanup in mapping to the pmap, but the pmap is allowed to arbitrarily unmap pages.
    pub unsafe fn free_page(&mut self, offset: VPN, orig: PPN) {
        if let Some(amap) = &self.amap
            && amap.get_page(offset).is_some()
        {
            // An anon is present; this page is not currently leased from the MemObject.
            return;
        }

        // TODO: Return orig to MemObject.
    }
}

/// Virtual address-space map.
pub struct VmSpace {
    /// Architecture-specific virtual to physical address map.
    pmap: PhysMap,
    /// Doubly-linked list of contiguous ranges with identical map and protection flags.
    /// The map may be modified in an unrelated place while a given entry is locked,
    /// so we opted for the raw pointer variant.
    map: Spinlock<InvasiveList<MapEntry>>,
}

impl VmSpace {
    /// Create a new mapping that must exist somewhere within `bounds` and try to place it at `hint`.
    /// If the `FIXED` flag is set in `map`, then overwrite any existing mappings in the way.
    /// Will never touch ranges of memory outside of `bounds`.
    /// TODO: MemObject parameter.
    pub unsafe fn map(
        &self,
        size: VPN,
        hint: VPN,
        bounds: Range<VPN>,
        map: u8,
        prot: u8,
    ) -> EResult<()> {
        todo!()
    }

    /// Change the protection flags for pages within `bounds`.
    /// No changes are made on failure.
    /// Preserves the outer portion of mappings on the border of `bounds`.
    pub unsafe fn protect(&self, bounds: Range<VPN>, prot: u8) -> EResult<()> {
        todo!()
    }

    /// Unmap all pages within `bounds`.
    /// No changes are made on failure.
    /// Preserves the outer portion of mappings on the border of `bounds`.
    pub unsafe fn unmap(&self, bounds: Range<VPN>, prot: u8) -> EResult<()> {
        todo!()
    }

    /// Handle a page fault at address `vaddr`.
    /// If this returns [`Ok`], the access should be retried.
    pub unsafe fn fault(&self, vaddr: usize) -> EResult<()> {
        todo!()
    }
}

impl Drop for VmSpace {
    fn drop(&mut self) {
        let mut map = self.map.lock();
        unsafe {
            while let Some(entry) = map.pop_front() {
                drop(Box::from_raw(entry));
            }
        }
    }
}
