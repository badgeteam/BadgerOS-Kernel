// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{fmt::Debug, ops::Range};

use alloc::{boxed::Box, sync::Arc, vec::Vec};

use crate::{
    bindings::error::{EResult, Errno},
    config::PAGE_SIZE,
    cpu::mmu,
    impl_has_list_node,
    kernel::sync::spinlock::Spinlock,
    mem::{
        pmm::{self, PAddrr},
        vmm::physmap::{PhysMap, Virt2Phys, higher_half_vaddr},
    },
    util::list::{InvasiveList, InvasiveListNode},
};

use super::{memobject::MemObject, physmap::canon_half_size, vmfence::VmFenceSet, *};

/// Mapping is shared (written back to memory object and not CoW'ed on fork).
pub const SHARED: u32 = 0x01;
/// Mapping is private (memory object read-only and CoW'ed on fork).
pub const PRIVATE: u32 = 0x02;
/// Replace the mapping at the given address if it exists.
pub const FIXED: u32 = 0x10;
/// Anonymous mapping (doesn't have an associated file descriptor).
pub const ANONYMOUS: u32 = 0x20;
/// Mapping must be populated immediately.
pub const POPULATE: u32 = 0x8000;
/// Kernel mapping need not be populated immediately; has no effect on user mappings.
/// If omitted, the mapping is forced to be [`SHARED`] and [`POPULATE`].
pub const LAZY_KERNEL: u32 = 0x8000_0000;

/// Specification for how to map a memory object.
#[derive(Clone, Debug)]
pub struct Mapping {
    /// Page-aligned byte offset within the memory object.
    pub offset: usize,
    /// The memory object to map.
    pub object: Arc<dyn MemObject>,
}

/// A region of anonymous memory.
#[derive(Debug)]
struct Anon {
    paddr: PAddrr,
}

#[derive(Clone, Debug)]
struct AnonMap {
    /// Page-aligned byte offset the parent [`MapEntry`].
    offset: usize,
    /// Resident pages of this region.
    pages: Vec<Option<Arc<Anon>>>,
}

impl AnonMap {
    /// Split this entry into two starting at `offset`.
    fn split(&self, offset: usize) -> EResult<(Option<Self>, Option<Self>)> {
        debug_assert!(offset % PAGE_SIZE as usize == 0);
        if offset < self.offset {
            return Ok((None, Some(self.clone())));
        }
        if offset == self.offset {
            return Ok((Some(self.clone()), None));
        }

        let mut first = Vec::try_with_capacity((offset - self.offset) / PAGE_SIZE as usize)?;
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
    fn get_page(&self, offset: usize) -> Option<PAddrr> {
        debug_assert!(offset % PAGE_SIZE as usize == 0);
        let page_index = offset.checked_sub(self.offset)? / PAGE_SIZE as usize;
        if page_index >= self.pages.len() {
            return None;
        }
        self.pages[page_index as usize].as_deref().map(|x| x.paddr)
    }

    /// Get or allocate the page at `offset`.
    /// If a new page is allocated, the existing data in the page at `orig` is copied.
    unsafe fn alloc_page(&mut self, offset: usize, orig: Option<PAddrr>) -> EResult<PAddrr> {
        debug_assert!(offset % PAGE_SIZE as usize == 0);
        if let Some(page) = self.get_page(offset) {
            return Ok(page);
        }

        if self.pages.len() == 0 {
            self.offset = offset;
            self.pages.try_reserve(1)?;
            self.pages.push(None);
        } else if offset < self.offset {
            let shift = (self.offset - offset) / PAGE_SIZE as usize;
            self.pages.try_reserve(shift)?;
            self.pages.splice(0..0, (0..shift + 1).map(|_| None));
            self.offset -= shift * PAGE_SIZE as usize;
        } else if offset >= self.offset + self.pages.len() * PAGE_SIZE as usize {
            let shift = offset + 1 - self.offset - self.pages.len() * PAGE_SIZE as usize;
            self.pages.try_reserve(shift)?;
            self.pages.resize(self.pages.len() + shift, None);
        }

        let new_page;
        let anon;
        unsafe {
            new_page = pmm::page_alloc(0, pmm::PageUsage::UserAnon)?;
            anon = match Arc::try_new(Anon { paddr: new_page }) {
                Ok(x) => x,
                Err(x) => {
                    pmm::page_free(new_page, 0);
                    return Err(x.into());
                }
            };

            let new_hhdm = new_page as usize + HHDM_OFFSET;
            if let Some(orig) = orig {
                let old_hhdm = orig as usize + HHDM_OFFSET;
                core::ptr::copy_nonoverlapping(
                    old_hhdm as *const u8,
                    new_hhdm as *mut u8,
                    PAGE_SIZE as usize,
                );
            } else {
                core::ptr::write_bytes(new_hhdm as *mut u8, 0, PAGE_SIZE as usize);
            }
        }

        self.pages[(offset - self.offset) / PAGE_SIZE as usize] = Some(anon);

        Ok(new_page)
    }
}

/// One contiguous range of mapped memory with the same protection and mapping flags.
struct MapEntry {
    node: InvasiveListNode,
    /// Region start and end virtual addresses.
    range: Range<usize>,
    /// Mapping dynamic state.
    inner: Spinlock<MapEntryInner>,
}
impl_has_list_node!(MapEntry, node);

/// One contiguous range of mapped memory with the same protection and mapping flags.
struct MapEntryInner {
    /// Region protection flags.
    prot_flags: u8,
    /// Region mapping flags.
    map_flags: u32,
    /// Anonymous memory overlay.
    amap: Option<Arc<AnonMap>>,
    /// Memory object mapped here.
    mapping: Option<Mapping>,
}

impl Debug for MapEntryInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MapEntryInner")
            .field("prot_flags", &self.prot_flags)
            .field("map_flags", &self.map_flags)
            .field("mapping", &self.mapping)
            .finish()
    }
}

impl MapEntryInner {
    /// Split this entry into two starting at `offset` within this mapping.
    fn split(&self, offset: usize) -> EResult<(Self, Self)> {
        debug_assert!(offset % PAGE_SIZE as usize == 0);
        let mut first = Self {
            prot_flags: self.prot_flags,
            map_flags: self.map_flags,
            amap: None,
            mapping: self.mapping.clone(),
        };
        let mut second = Self {
            prot_flags: self.prot_flags,
            map_flags: self.map_flags,
            amap: None,
            mapping: self.mapping.clone(),
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
    fn get_page(&self, offset: usize, allow_writing: Option<&mut bool>) -> Option<PAddrr> {
        debug_assert!(offset % PAGE_SIZE as usize == 0);
        if let Some(amap) = &self.amap
            && let Some(page) = amap.get_page(offset)
        {
            if let Some(allow_writing) = allow_writing {
                *allow_writing = true;
            }
            return Some(page);
        }

        if let Some(mapping) = &self.mapping {
            if let Some(allow_writing) = allow_writing {
                *allow_writing = self.map_flags & SHARED == 0;
            }
            mapping.object.get(offset + mapping.offset)
        } else {
            if let Some(allow_writing) = allow_writing {
                *allow_writing = false;
            }
            Some(zeroes_paddr())
        }
    }

    /// Get or allocate the page at `offset`.
    unsafe fn alloc_page(
        &mut self,
        offset: usize,
        for_writing: bool,
        allow_writing: Option<&mut bool>,
    ) -> EResult<PAddrr> {
        debug_assert!(offset % PAGE_SIZE as usize == 0);
        if let Some(amap) = &self.amap
            && let Some(page) = amap.get_page(offset)
        {
            if let Some(allow_writing) = allow_writing {
                *allow_writing = true;
            }
            return Ok(page);
        }

        // Page is not cached, get it from the memory object.
        let paddr;
        if let Some(mapping) = &self.mapping {
            paddr = mapping.object.alloc(offset + mapping.offset)?;
        } else {
            paddr = zeroes_paddr();
        }

        if !for_writing || self.map_flags & SHARED == 0 {
            // Page can be immediately mapped given for the given access type.
            if let Some(allow_writing) = allow_writing {
                *allow_writing = self.map_flags & SHARED == 0;
            }
            return Ok(paddr);
        }

        // Page can't be immediately mapped; instead, it shall be copied to a new anon.
        // However, we don't do this for the page of zeroes so we know to memset instead of memcpy later.
        let orig = self.mapping.is_some().then_some(paddr);

        // Ensure we have a mutable reference to an AnonMap.
        if let Some(amap) = &mut self.amap {
            if !Arc::is_unique(amap) {
                // Duplicating the AnonMap here so private changes are copied on write.
                *amap = Arc::try_new((**amap).clone())?;
            }
        } else {
            // Initial creation of the AnonMap.
            self.amap = Some(Arc::try_new(AnonMap {
                offset: 0,
                pages: Vec::new(),
            })?);
        }
        let amap = Arc::get_mut(self.amap.as_mut().unwrap()).unwrap();

        // SAFETY: `orig` here comes from the memory object, which promises it is valid physical memory.
        unsafe { amap.alloc_page(offset, orig) }
    }
}

/// Virtual address-space map.
pub(super) struct VmSpaceInner {
    /// Architecture-specific virtual to physical address map.
    pub(super) pmap: PhysMap,
    /// Doubly-linked list of contiguous ranges with identical map and protection flags.
    /// The map may be modified in an unrelated place while a given entry is locked,
    /// so we opted for the raw pointer variant.
    map: Spinlock<InvasiveList<MapEntry>>,
}

impl VmSpaceInner {
    /// Margin in bytes between mappings that [`Self::map`] guarantees.
    pub const MAP_MARGIN: usize = PAGE_SIZE as usize;

    /// Create a new, empty address space.
    pub(super) fn new(pmap: PhysMap) -> Self {
        Self {
            pmap,
            map: Spinlock::new(InvasiveList::new()),
        }
    }

    /// Try to split the mappings so that they do not cross `threshold`.
    /// Used to implement the splitting logic used by the various manipulation functions.
    fn split(threshold: usize, map: &mut InvasiveList<MapEntry>) -> EResult<()> {
        unsafe {
            debug_assert!(threshold % PAGE_SIZE as usize == 0);
            let mut cur = map.front();
            while let Some(entry) = cur {
                let mut guard = (&*entry).inner.lock();
                if (*entry).range.start < threshold && (*entry).range.end > threshold {
                    let (first, second) = guard.split(threshold - (*entry).range.start)?;
                    let new = Box::into_raw(Box::try_new(MapEntry {
                        range: threshold..(*entry).range.end,
                        node: InvasiveListNode::new(),
                        inner: Spinlock::new(second),
                    })?);
                    (*entry).range.end = threshold;
                    *guard = first;
                    map.insert_after(entry, new);
                }
                drop(guard);
                cur = map.next(entry);
            }
            Ok(())
        }
    }

    /// Removes entries within `bounds`.
    unsafe fn remove_mappings(
        fences: &mut VmFenceSet,
        pmap: &PhysMap,
        map: &mut InvasiveList<MapEntry>,
        bounds: Range<usize>,
    ) {
        let mut deferred_free = Vec::new();
        unsafe {
            debug_assert!(bounds.start % PAGE_SIZE as usize == 0);
            debug_assert!(bounds.end % PAGE_SIZE as usize == 0);
            let mut cur = map.front();
            while let Some(entry) = cur {
                let next = map.next(entry);
                if (*entry).range.start >= bounds.end {
                    break;
                } else if bounds.contains(&(*entry).range.start) {
                    // Lock here to ensure no other thread is still e.g. performing fault() on it.
                    drop((*entry).inner.lock());
                    pmap.unmap_multiple((*entry).range.start, (*entry).range.len());
                    map.remove(entry);
                    // Schedule VM fences for all affected pages.
                    for vaddr in (*entry).range.clone() {
                        fences.add(Some(vaddr), None);
                    }
                    // Can't free the entry until VM fence is done; this vector defers it to the end of the function.
                    deferred_free.push(Box::from_raw(entry));
                }
                cur = next;
            }
        }
        // Do TLB shootdown just before `deferred_free` gets dropped at the end.
        vmfence::shootdown(fences);
        // Fences performed, no reason to do double work later.
        fences.clear();
    }

    /// Insert a new mapping.
    unsafe fn insert_mapping(map: &mut InvasiveList<MapEntry>, insert: Box<MapEntry>) {
        unsafe {
            debug_assert!(insert.range.start % PAGE_SIZE as usize == 0);
            debug_assert!(insert.range.end % PAGE_SIZE as usize == 0);
            if map.len() == 0 {
                map.push_front(Box::into_raw(insert)).unwrap();
                return;
            }

            let mut prev = 0;
            let mut cur = map.front();
            while let Some(entry) = cur {
                let next = map.next(entry);
                if let Some(next) = next {
                    debug_assert!((*next).range.start >= (*entry).range.end);
                }
                if prev <= insert.range.start && (*entry).range.start >= insert.range.end {
                    debug_assert!(insert.range.end <= (*entry).range.start);
                    map.insert_before(entry, Box::into_raw(insert));
                    return;
                }
                prev = (*entry).range.end;
                cur = next;
            }

            map.push_back(Box::into_raw(insert)).unwrap();
        }
    }

    /// Implementation of [`Self::map`] with the [`FIXED`] flag.
    unsafe fn map_fixed(
        fences: &mut VmFenceSet,
        pmap: &PhysMap,
        map: &mut InvasiveList<MapEntry>,
        size: usize,
        addr: usize,
        map_flags: u32,
        prot_flags: u8,
        mapping: Option<Mapping>,
    ) -> EResult<()> {
        debug_assert!(addr % PAGE_SIZE as usize == 0);
        debug_assert!(size % PAGE_SIZE as usize == 0);
        Self::split(addr, map)?;
        Self::split(addr + size, map)?;

        let entry = Box::try_new(MapEntry {
            node: InvasiveListNode::new(),
            range: addr..addr + size,
            inner: Spinlock::new(MapEntryInner {
                prot_flags,
                map_flags,
                amap: None,
                mapping,
            }),
        })?;
        unsafe {
            Self::remove_mappings(fences, pmap, map, addr..addr + size);
            Self::insert_mapping(map, entry);
        }

        Ok(())
    }

    /// Implementation of [`Self::map`] without the [`FIXED`] flag.
    unsafe fn map_dynamic(
        map: &mut InvasiveList<MapEntry>,
        size: usize,
        mut hint: usize,
        mut bounds: Range<usize>,
        map_flags: u32,
        prot_flags: u8,
        mapping: Option<Mapping>,
    ) -> EResult<usize> {
        debug_assert!(hint % PAGE_SIZE as usize == 0);
        debug_assert!(size % PAGE_SIZE as usize == 0);
        debug_assert!(bounds.start % PAGE_SIZE as usize == 0);
        debug_assert!(bounds.end % PAGE_SIZE as usize == 0);
        // Constrain bounds to the closest range that would fit the mapping.
        if map.len() > 0 {
            let mut closest: Option<usize> = None;
            let mut closest_size = 0;
            let mut prev = bounds.start;
            for entry in unsafe { map.iter() } {
                if let Some(closest) = closest
                    && prev.abs_diff(hint) > closest.abs_diff(hint)
                {
                    break;
                }
                let start = entry.range.start.max(bounds.start);
                if start >= prev + size + 2 * Self::MAP_MARGIN {
                    closest = Some(prev);
                    closest_size = start - prev;
                }
                prev = entry.range.end.max(bounds.start);
            }
            if closest.is_none() || prev.abs_diff(hint) <= closest.unwrap().abs_diff(hint) {
                if bounds.end >= prev + size {
                    closest = Some(prev);
                    closest_size = bounds.end - prev;
                }
            }

            let closest = closest.ok_or(Errno::ENOMEM)?;
            bounds.start = bounds.start.max(closest);
            bounds.end = bounds.end.min(closest + closest_size);
        }

        if bounds.len() < size + 2 * Self::MAP_MARGIN {
            return Err(Errno::ENOMEM);
        } else if hint < bounds.start + Self::MAP_MARGIN {
            hint = bounds.start + Self::MAP_MARGIN;
        } else if hint.saturating_add(size) > bounds.end - Self::MAP_MARGIN {
            hint = bounds.end - size - Self::MAP_MARGIN;
        }

        let entry = Box::try_new(MapEntry {
            node: InvasiveListNode::new(),
            range: hint..hint + size,
            inner: Spinlock::new(MapEntryInner {
                prot_flags,
                map_flags,
                amap: None,
                mapping,
            }),
        })?;
        unsafe {
            Self::insert_mapping(map, entry);
        }

        Ok(hint)
    }

    /// Create a new mapping that must exist somewhere within `bounds` and try to place it at `hint`.
    /// If the `FIXED` flag is set in `map`, then overwrite any existing mappings in the way.
    /// Will never touch ranges of memory outside of `bounds`.
    /// TODO: MemObject parameter.
    pub unsafe fn map(
        &self,
        size: usize,
        hint: usize,
        bounds: Range<usize>,
        map_flags: u32,
        prot_flags: u8,
        mapping: Option<Mapping>,
    ) -> EResult<usize> {
        let has_mapping = mapping.is_some();
        assert!(hint % PAGE_SIZE as usize == 0);
        let size = size.div_ceil(PAGE_SIZE as usize) * PAGE_SIZE as usize;
        unsafe {
            let mut fences = VmFenceSet::new();
            let mut map = self.map.lock();
            let addr = if map_flags & FIXED != 0 {
                assert!(hint % PAGE_SIZE as usize == 0);
                Self::map_fixed(
                    &mut fences,
                    &self.pmap,
                    &mut map,
                    size,
                    hint,
                    map_flags,
                    prot_flags,
                    mapping,
                )?;
                hint
            } else {
                Self::map_dynamic(&mut map, size, hint, bounds, map_flags, prot_flags, mapping)?
            };
            let _map = map.demote();
            if map_flags & POPULATE != 0 {
                let access = if !has_mapping {
                    prot::WRITE
                } else {
                    prot::READ
                };
                for addr in (addr..addr + size).step_by(PAGE_SIZE as usize) {
                    self.fault(&mut fences, addr, access)
                        .expect("Prefault failed");
                }
            }
            vmfence::shootdown(&fences);
            Ok(addr)
        }
    }

    /// Change the protection flags for pages within `bounds`.
    /// No changes are made on failure.
    /// Preserves the outer portion of mappings on the border of `bounds`.
    pub unsafe fn protect(&self, bounds: Range<usize>, prot_flags: u8) -> EResult<()> {
        debug_assert!(bounds.start % PAGE_SIZE as usize == 0);
        debug_assert!(bounds.end % PAGE_SIZE as usize == 0);
        let mut map = self.map.lock();
        Self::split(bounds.start, &mut map)?;
        Self::split(bounds.end, &mut map)?;

        let mut fences = VmFenceSet::new();

        // TODO: Go over entries, change their prot flags, protect them on the pmap. Then, do TLB shootdown.

        todo!()
    }

    /// Unmap all pages within `bounds`.
    /// No changes are made on failure.
    /// Preserves the outer portion of mappings on the border of `bounds`.
    pub unsafe fn unmap(&self, bounds: Range<usize>) -> EResult<()> {
        debug_assert!(bounds.start % PAGE_SIZE as usize == 0);
        debug_assert!(bounds.end % PAGE_SIZE as usize == 0);
        let mut map = self.map.lock();
        Self::split(bounds.start, &mut map)?;
        Self::split(bounds.end, &mut map)?;

        let mut fences = VmFenceSet::new();
        unsafe {
            Self::remove_mappings(&mut fences, &self.pmap, &mut map, bounds);
        }
        vmfence::shootdown(&fences);

        Ok(())
    }

    /// Implementation of [`Self::fault`] if the entry is found.
    fn fault_impl(
        fences: &mut VmFenceSet,
        pmap: &PhysMap,
        vaddr: usize,
        access: u8,
        entry: &MapEntry,
    ) -> EResult<()> {
        let page_vaddr = vaddr - vaddr % PAGE_SIZE as usize;
        let v2p = pmap.virt2phys(page_vaddr);
        let flags = if v2p.valid {
            prot::from_mmu_flags(v2p.flags)
        } else {
            0
        };
        if flags >= access {
            // TLB must be outdated; flush it and retry.
            mmu::vmem_fence(Some(page_vaddr), None);
            return Ok(());
        }

        if v2p.valid {
            // SAFETY: The PhysMap is disposable, so discarding from it temporarily is legal.
            unsafe {
                pmap.unmap(page_vaddr);
            }
            // Since we're going to change the mapping, we must ensure that no stale copy remains in any TLBs.
            fences.add(Some(page_vaddr), None);
            vmfence::shootdown(fences);
            fences.clear();
        }

        // Get the page from either the cache or the memory object.
        let mut guard = entry.inner.lock();
        let mut allow_writing = false;
        let existing = guard.get_page(page_vaddr - entry.range.start, Some(&mut allow_writing));
        let paddr;
        if existing.is_none() || access == prot::WRITE && guard.map_flags & SHARED != 0 {
            // SAFETY: We know the existing page to be owned by the same range, so it is readable.
            paddr = unsafe {
                guard.alloc_page(
                    page_vaddr - entry.range.start,
                    access == prot::WRITE,
                    Some(&mut allow_writing),
                )?
            };
        } else {
            paddr = existing.unwrap();
        }

        // SAFETY: The page mapped here is provided by the range and we need to trust it is correct.
        unsafe {
            let mut prot_flags = guard.prot_flags;
            if !allow_writing && guard.map_flags & SHARED == 0 {
                prot_flags &= !prot::WRITE;
            }
            pmap.map(page_vaddr, paddr, prot::into_mmu_flags(prot_flags))?;
        }

        Ok(())
    }

    /// Handle a page fault at address `vaddr`.
    /// If this returns [`Ok`], the access should be retried.
    pub fn fault(&self, fences: &mut VmFenceSet, vaddr: usize, access: u8) -> EResult<()> {
        let map = self.map.lock_shared();

        for entry in unsafe { map.iter() } {
            if entry.range.contains(&vaddr) {
                return Self::fault_impl(fences, &self.pmap, vaddr, access, entry);
            }
        }

        Err(Errno::EFAULT)
    }

    /// Clear the entire address-space.
    pub fn clear(&self) {
        todo!()
    }
}

impl Drop for VmSpaceInner {
    fn drop(&mut self) {
        let mut map = self.map.lock();
        unsafe {
            while let Some(entry) = map.pop_front() {
                drop(Box::from_raw(entry));
            }
        }
    }
}

impl Debug for VmSpaceInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let map = self.map.lock_shared();
        let mut pretty = f.debug_map();

        unsafe {
            for ent in map.iter() {
                pretty.entry(&ent.range, ent.inner.lock_shared().deref());
            }
        }

        pretty.finish()
    }
}

/// Virtual address-space map for user memory.
pub struct VmSpace(VmSpaceInner);

impl VmSpace {
    /// Create a new address-space for user memory.
    pub fn new() -> EResult<Self> {
        let pmap = PhysMap::new()?;
        unsafe {
            PhysMap::broadcast_higher_half(&kernel_mm().0.pmap, &pmap);
        }
        Ok(Self(VmSpaceInner::new(pmap)))
    }

    /// The range within which the kernel will allow new non-fixed mmaps to be created for userspace.
    pub fn bounds() -> Range<usize> {
        canon_half_size() / 2..canon_half_size()
    }

    /// Create a new mapping that must exist somewhere within `bounds` and try to place it at `hint`.
    /// If the `FIXED` flag is set in `map`, then overwrite any existing mappings in the way.
    /// Will never touch ranges of memory outside of `bounds`.
    /// TODO: MemObject parameter.
    pub fn map(
        &self,
        size: usize,
        hint: usize,
        map_flags: u32,
        prot_flags: u8,
        mapping: Option<Mapping>,
    ) -> EResult<usize> {
        // Assert page-aligned hints.
        if hint % PAGE_SIZE as usize != 0 {
            return Err(Errno::EINVAL);
        }
        // Assert the virtual addresses are in the lower half.
        if map_flags & FIXED != 0
            && (hint == 0 || hint.saturating_add(size) >= canon_half_size() - PAGE_SIZE as usize)
        {
            return Err(Errno::EINVAL);
        }
        unsafe {
            self.0.map(
                size,
                hint,
                Self::bounds(),
                map_flags,
                prot_flags & !prot::IO & !prot::NC,
                mapping,
            )
        }
    }

    /// Change the protection flags for pages within `bounds`.
    /// No changes are made on failure.
    /// Preserves the outer portion of mappings on the border of `bounds`.
    pub fn protect(&self, bounds: Range<usize>, prot_flags: u8) -> EResult<()> {
        assert!(Self::bounds().start <= bounds.start && Self::bounds().end >= bounds.end);
        unsafe { self.0.protect(bounds, prot_flags) }
    }

    /// Unmap all pages within `bounds`.
    /// No changes are made on failure.
    /// Preserves the outer portion of mappings on the border of `bounds`.
    pub fn unmap(&self, bounds: Range<usize>) -> EResult<()> {
        assert!(Self::bounds().start <= bounds.start && Self::bounds().end >= bounds.end);
        unsafe { self.0.unmap(bounds) }
    }

    /// Handle a page fault at address `vaddr`.
    /// If this returns [`Ok`], the access should be retried.
    pub fn fault(&self, vaddr: usize, access: u8) -> EResult<()> {
        let mut fences = VmFenceSet::new();
        self.0.fault(&mut fences, vaddr, access)?;
        Ok(())
    }

    /// Clone this virtual-address space as needed for the `fork` system call.
    pub fn fork(&self) -> EResult<Self> {
        todo!()
    }

    /// Clear the entire address-space.
    pub fn clear(&self) {
        self.0.clear();
    }

    /// Enable the underlying physical map on this CPU.
    pub unsafe fn enable(&self) {
        unsafe {
            self.0.pmap.enable();
        }
    }

    /// Do a virtual to physical address lookup.
    pub fn virt2phys(&self, vaddr: usize) -> Virt2Phys {
        self.0.pmap.virt2phys(vaddr)
    }
}

impl Debug for VmSpace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

/// Virtual address-space map for user memory.
pub struct KernelVmSpace(pub(super) VmSpaceInner);

impl KernelVmSpace {
    pub(super) fn new() -> Self {
        let pmap = PhysMap::new().expect("Failed to allocate kernel PhysMap");
        unsafe {
            pmap.populate_higher_half()
                .expect("Failed to allocate kernel PhysMap");
        }
        Self(VmSpaceInner::new(pmap))
    }

    /// The bounds within which the kernel makes its own non-fixed mappings.
    pub fn bounds() -> Range<usize> {
        // The upper quarter of the higher half is used for miscellaneous mappings.
        higher_half_vaddr() + canon_half_size() / 4..(PAGE_SIZE as usize).wrapping_neg()
    }

    /// Create a new mapping that must exist somewhere within `bounds` and try to place it at `hint`.
    /// If the `FIXED` flag is set in `map`, then overwrite any existing mappings in the way.
    /// Will never touch ranges of memory outside of `bounds`.
    /// TODO: MemObject parameter.
    pub unsafe fn map(
        &self,
        size: usize,
        hint: usize,
        mut map_flags: u32,
        prot_flags: u8,
        mapping: Option<Mapping>,
    ) -> EResult<usize> {
        assert!(hint % PAGE_SIZE as usize == 0);
        if map_flags & FIXED != 0 {
            assert!(hint >= higher_half_vaddr());
        }
        if map_flags & LAZY_KERNEL == 0 {
            map_flags |= POPULATE;
            map_flags &= !PRIVATE;
            map_flags |= SHARED;
        }
        unsafe {
            self.0
                .map(size, hint, Self::bounds(), map_flags, prot_flags, mapping)
        }
    }

    /// Change the protection flags for pages within `bounds`.
    /// No changes are made on failure.
    /// Preserves the outer portion of mappings on the border of `bounds`.
    pub unsafe fn protect(&self, bounds: Range<usize>, prot_flags: u8) -> EResult<()> {
        assert!(Self::bounds().start <= bounds.start && Self::bounds().end >= bounds.end);
        unsafe { self.0.protect(bounds, prot_flags) }
    }

    /// Unmap all pages within `bounds`.
    /// No changes are made on failure.
    /// Preserves the outer portion of mappings on the border of `bounds`.
    pub unsafe fn unmap(&self, bounds: Range<usize>) -> EResult<()> {
        assert!(Self::bounds().start <= bounds.start && Self::bounds().end >= bounds.end);
        unsafe { self.0.unmap(bounds) }
    }

    /// Handle a page fault at address `vaddr`.
    /// If this returns [`Ok`], the access should be retried.
    pub fn fault(&self, vaddr: usize, access: u8) -> EResult<()> {
        let mut fences = VmFenceSet::new();
        self.0.fault(&mut fences, vaddr, access)?;
        Ok(())
    }

    /// Enable the underlying physical map on this CPU.
    pub fn enable(&self) {
        unsafe {
            self.0.pmap.enable();
        }
    }

    /// Do a virtual to physical address lookup.
    pub fn virt2phys(&self, vaddr: usize) -> Virt2Phys {
        self.0.pmap.virt2phys(vaddr)
    }
}

impl Debug for KernelVmSpace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}
