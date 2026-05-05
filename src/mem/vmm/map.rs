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
        vmm::physmap::{PhysMap, Virt2Phys, higher_half_vaddr, is_canon_user_range},
    },
    util::list::{InvasiveList, InvasiveListNode},
};

use super::{memobject::MemObject, physmap::canon_half_size, prot::WRITE, vmfence::VmFenceSet, *};

/// Mapping is shared (written back to memory object and not CoW'ed on fork).
pub const SHARED: u32 = 0x01;
/// Mapping is private (memory object read-only and CoW'ed on fork).
pub const PRIVATE: u32 = 0x02;
/// Replace the mapping at the given address if it exists.
pub const FIXED: u32 = 0x10;
/// Anonymous mapping (doesn't have an associated file descriptor).
pub const ANONYMOUS: u32 = 0x20;
/// Deny writes (as done by exec).
pub const DENYWRITE: u32 = 0x40;
/// Mapping must be populated immediately.
pub const POPULATE: u32 = 0x8000;
/// Kernel mapping need not be populated immediately; has no effect on user mappings.
/// If omitted, the mapping is forced to be [`SHARED`] and [`POPULATE`].
pub const LAZY_KERNEL: u32 = 0x8000_0000;

/// Specification for how to map a memory object.
#[derive(Clone, Debug)]
pub struct Mapping {
    /// Page-aligned byte offset within the memory object.
    pub offset: u64,
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
    fn get_page(&self, offset: usize) -> Option<MappablePage> {
        debug_assert!(offset % PAGE_SIZE as usize == 0);
        let page_index = offset.checked_sub(self.offset)? / PAGE_SIZE as usize;
        if page_index >= self.pages.len() {
            return None;
        }
        let arc_ref = self.pages[page_index as usize].as_ref()?;
        let writable = Arc::is_unique(arc_ref);
        Some(unsafe { MappablePage::new(arc_ref.paddr, false, writable, false) })
    }

    /// Get or allocate the page at `offset`.
    /// If a new page is allocated, the existing data in the page at `orig` is copied.
    unsafe fn alloc_page(&mut self, offset: usize, orig: Option<PAddrr>) -> EResult<MappablePage> {
        debug_assert!(offset % PAGE_SIZE as usize == 0);

        if self.pages.len() == 0 {
            self.offset = offset;
            self.pages.try_reserve(1)?;
            self.pages.push(None);
        } else if offset < self.offset {
            let shift = (self.offset - offset) / PAGE_SIZE as usize;
            self.pages.try_reserve(shift)?;
            self.pages.splice(0..0, (0..shift).map(|_| None));
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

        Ok(unsafe { MappablePage::new(new_page, false, true, false) })
    }
}

/// One contiguous range of mapped memory with the same protection and mapping flags.
pub struct MapEntry {
    node: InvasiveListNode,
    /// Region start and end virtual addresses.
    range: Range<usize>,
    /// Mapping dynamic state.
    inner: Spinlock<MapEntryInner>,
}
impl_has_list_node!(MapEntry, node);

/// One contiguous range of mapped memory with the same protection and mapping flags.
#[derive(Clone)]
pub struct MapEntryInner {
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
    fn get_page(&self, offset: usize) -> Option<MappablePage> {
        debug_assert!(offset % PAGE_SIZE as usize == 0);
        if let Some(amap) = &self.amap
            && let Some(mut page) = amap.get_page(offset)
        {
            // Only writable if this is the sole owner; otherwise CoW is still pending.
            if !Arc::is_unique(amap) {
                page.clear_writable();
            }

            return Some(page);
        }

        if let Some(mapping) = &self.mapping {
            let mut page = mapping.object.get(offset as u64 + mapping.offset)?;
            if self.map_flags & SHARED == 0 {
                // Private mappings; can't directly write to the memory object.
                page.clear_writable();
            }
            Some(page)
        } else {
            Some(zeroes_page())
        }
    }

    /// Get or allocate the page at `offset`.
    unsafe fn alloc_page(&mut self, offset: usize, for_writing: bool) -> EResult<MappablePage> {
        debug_assert!(offset % PAGE_SIZE as usize == 0);

        let orig;
        if let Some(mut page) = try { self.amap.as_ref()?.get_page(offset)? } {
            // An anon already exists for this page. Must also check AnonMap uniqueness,
            // not just Arc<Anon> uniqueness; a shared AnonMap means CoW is still pending.
            if !Arc::is_unique(self.amap.as_ref().unwrap()) {
                page.clear_writable();
            }
            if !for_writing || page.writable() {
                return Ok(page);
            }
            orig = Some(page);
        } else {
            // Page is not cached, get it from the memory object.
            let mut paddr;
            if let Some(mapping) = &self.mapping {
                paddr = mapping.object.alloc(offset as u64 + mapping.offset)?;
            } else {
                paddr = zeroes_page();
            }

            if !for_writing || ((self.map_flags & SHARED) != 0 && self.mapping.is_some()) {
                // Page can be immediately mapped given for the given access type.
                if (self.map_flags & SHARED) == 0 {
                    paddr.clear_writable();
                }
                return Ok(paddr);
            }

            // Page can't be immediately mapped; instead, it shall be copied to a new anon.
            // However, we don't do this for the page of zeroes so we know to memset instead of memcpy later.
            orig = self.mapping.is_some().then_some(paddr);
        }

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
        unsafe { amap.alloc_page(offset, orig.as_ref().map(MappablePage::paddr)) }
    }
}

/// Virtual address-space map.
pub struct VmSpaceInner {
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
        vmspace: *const VmSpaceInner,
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
                    let inner = (*entry).inner.lock();
                    if let Some(m) = &inner.mapping {
                        m.object
                            .on_unmapped(inner.map_flags & DENYWRITE != 0, vmspace, entry);
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
        vmspace: *const VmSpaceInner,
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
        let entry_ptr = entry.as_ref() as *const MapEntry;
        unsafe {
            Self::remove_mappings(fences, pmap, map, addr..addr + size, vmspace);
            Self::insert_mapping(map, entry);
            let inner = (*entry_ptr).inner.lock();
            if let Some(m) = &inner.mapping {
                let obj = m.object.clone();
                if let Err(e) = obj.on_mapped(inner.map_flags & DENYWRITE != 0, vmspace, entry_ptr)
                {
                    // on_mapped failed after FIXED already cleared the old range; a hole is left.
                    map.remove(entry_ptr as *mut _);
                    drop(Box::from_raw(entry_ptr as *mut MapEntry));
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Implementation of [`Self::map`] without the [`FIXED`] flag.
    unsafe fn map_dynamic(
        map: &mut InvasiveList<MapEntry>,
        vmspace: *const VmSpaceInner,
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
        let entry_ptr = entry.as_ref() as *const MapEntry;
        unsafe {
            Self::insert_mapping(map, entry);
            if let Some(m) = &(*entry_ptr).inner.lock().mapping {
                let obj = m.object.clone();
                if let Err(e) = obj.on_mapped(map_flags & DENYWRITE != 0, vmspace, entry_ptr) {
                    map.remove(entry_ptr as *mut _);
                    drop(Box::from_raw(entry_ptr as *mut MapEntry));
                    return Err(e);
                }
            }
        }

        Ok(hint)
    }

    /// Create a new mapping that must exist somewhere within `bounds` and try to place it at `hint`.
    /// If the `FIXED` flag is set in `map`, then overwrite any existing mappings in the way.
    /// Will never touch ranges of memory outside of `bounds`.
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

        if size == 0 {
            return Err(Errno::EINVAL);
        }
        if let Some(mapping) = &mapping {
            let mapping_size = (mapping.object.len() - mapping.offset).div_ceil(PAGE_SIZE as u64)
                * PAGE_SIZE as u64;
            if size as u64 > mapping_size {
                return Err(Errno::ENXIO);
            }
        }

        unsafe {
            let mut fences = VmFenceSet::new();
            let mut map = self.map.lock();
            let addr = if map_flags & FIXED != 0 {
                assert!(hint % PAGE_SIZE as usize == 0);
                Self::map_fixed(
                    &mut fences,
                    &self.pmap,
                    &mut map,
                    self as *const _,
                    size,
                    hint,
                    map_flags,
                    prot_flags,
                    mapping,
                )?;
                hint
            } else {
                Self::map_dynamic(
                    &mut map,
                    self as *const _,
                    size,
                    hint,
                    bounds,
                    map_flags,
                    prot_flags,
                    mapping,
                )?
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

        unsafe {
            debug_assert!(bounds.start % PAGE_SIZE as usize == 0);
            debug_assert!(bounds.end % PAGE_SIZE as usize == 0);
            let mut cur = map.front();
            while let Some(entry) = cur {
                let next = map.next(entry);
                if (*entry).range.start >= bounds.end {
                    break;
                } else if bounds.contains(&(*entry).range.start) {
                    let mut guard = (*entry).inner.lock();
                    guard.prot_flags = prot_flags;
                    self.pmap.protect(
                        (*entry).range.start,
                        (*entry).range.len(),
                        prot::into_mmu_flags(prot_flags),
                    );
                    // Schedule VM fences for all affected pages.
                    for vaddr in (*entry).range.clone() {
                        fences.add(Some(vaddr), None);
                    }
                }
                cur = next;
            }
        }
        // Do TLB shootdown to protect against stale TLB entries with higher mapping permission flags.
        vmfence::shootdown(&fences);

        Ok(())
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
            Self::remove_mappings(&mut fences, &self.pmap, &mut map, bounds, self as *const _);
        }
        vmfence::shootdown(&fences);

        Ok(())
    }

    /// Evict entries from the pmaps and anons that are beyond the new length.
    /// Called by memory objects that have been resized to become smaller.
    pub unsafe fn shrink(&self, entry: *const MapEntry, max_len: u64) {
        let entry = unsafe { &*entry };
        let entry_len = entry.range.end - entry.range.start;

        // Anons detached from the amap must not be dropped until after the TLB shootdown,
        // because the pmap does not shoot down TLBs itself.
        let mut deferred: Vec<Arc<Anon>> = Vec::new();

        let cutoff_offset = {
            let mut inner = entry.inner.lock();

            let m = match &inner.mapping {
                Some(m) => m,
                None => return,
            };

            // Bytes of the file that still fall within this entry's range.
            let valid = max_len.saturating_sub(m.offset).min(entry_len as u64) as usize;
            // Round up to a page boundary: the partial last page stays because the page cache
            // already zeroed its tail; only pages lying entirely beyond max_len are evicted.
            let cutoff = valid.div_ceil(PAGE_SIZE as usize) * PAGE_SIZE as usize;

            if cutoff < entry_len {
                // Evict pmap entries while holding the inner lock so a concurrent fault
                // cannot re-install a page we just removed (fault_impl locks inner first).
                unsafe {
                    self.pmap
                        .unmap_multiple(entry.range.start + cutoff, entry_len - cutoff);
                }

                // Detach anon pages at and beyond the cutoff into `deferred`; they must
                // not actually be freed until the TLB shootdown below.
                let drop_all = inner.amap.as_ref().map_or(false, |a| cutoff <= a.offset);
                if drop_all {
                    if let Some(amap) = inner.amap.take() {
                        // If unique, drain pages into deferred. If shared (CoW clone), the
                        // Arc drop is safe — the other clone still holds the physical pages.
                        if let Ok(mut unique) = Arc::try_unwrap(amap) {
                            for slot in unique.pages.drain(..) {
                                if let Some(anon) = slot {
                                    deferred.push(anon);
                                }
                            }
                        }
                    }
                } else if let Some(amap) = inner.amap.as_mut() {
                    let amap = Arc::make_mut(amap);
                    let start_page = (cutoff - amap.offset) / PAGE_SIZE as usize;
                    for slot in &mut amap.pages[start_page..] {
                        if let Some(anon) = slot.take() {
                            deferred.push(anon);
                        }
                    }
                }
            }

            cutoff
        };

        if cutoff_offset >= entry_len {
            return;
        }

        let mut fences = VmFenceSet::new();
        for vaddr in
            (entry.range.start + cutoff_offset..entry.range.end).step_by(PAGE_SIZE as usize)
        {
            fences.add(Some(vaddr), None);
        }
        vmfence::shootdown(&fences);
        // TLB shootdown complete; safe to free the detached anon pages now.
        drop(deferred);
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
        if flags & access == access {
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

        let mut guard = entry.inner.lock();
        if (guard.prot_flags & access) == 0 {
            // No permission.
            return Err(Errno::EFAULT);
        }

        // Get the page from either the cache or the memory object.
        let existing = guard.get_page(page_vaddr - entry.range.start);
        let page;
        if existing.is_none() || (access == prot::WRITE && !existing.as_ref().unwrap().writable()) {
            // SAFETY: We know the existing page to be owned by the same range, so it is readable.
            page =
                unsafe { guard.alloc_page(page_vaddr - entry.range.start, access == prot::WRITE)? };
        } else {
            page = existing.unwrap();
        }

        // SAFETY: The page mapped here is provided by the range and we need to trust it is correct.
        unsafe {
            let mut prot_flags = guard.prot_flags;
            if !page.writable() && guard.map_flags & SHARED == 0 {
                prot_flags &= !prot::WRITE;
            }
            let mut mmu_flags = prot::into_mmu_flags(prot_flags) | physmap::flags::A;
            if vaddr as isize > 0 {
                mmu_flags |= physmap::flags::U;
            } else {
                mmu_flags |= physmap::flags::G;
            }
            if page.refcounted() {
                mmu_flags |= physmap::flags::REFCOUNT;
            }
            if !page.tracks_dirty() {
                mmu_flags |= physmap::flags::D;
            }
            pmap.map(page_vaddr, page.into_paddr(), mmu_flags)?;
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

    /// Clone this virtual-address space as needed for the `fork` system call.
    pub fn fork(&self) -> EResult<Self> {
        let map = self.map.lock();
        let mut fences = VmFenceSet::new();
        let mut new_map = InvasiveList::new();

        unsafe {
            for entry in map.iter() {
                let inner = (*entry).inner.lock();
                let new_inner = inner.clone();

                if (inner.map_flags & PRIVATE) != 0 && (inner.prot_flags & WRITE) != 0 {
                    // Writable private mappings need to be CoW'ed.
                    self.pmap.protect(
                        entry.range.start,
                        entry.range.len(),
                        prot::into_mmu_flags(prot::READ | prot::EXEC),
                    );
                    for vaddr in entry.range.clone() {
                        fences.add(Some(vaddr), None);
                    }
                }

                let new_entry = MapEntry {
                    node: InvasiveListNode::new(),
                    range: entry.range.clone(),
                    inner: Spinlock::new(new_inner),
                };
                let _ = new_map.push_back(Box::into_raw(Box::try_new(new_entry)?));
            }
        }

        vmfence::shootdown(&fences);

        let pmap = PhysMap::new()?;
        unsafe { PhysMap::broadcast_higher_half(&kernel_mm().0.pmap, &pmap) };
        Ok(Self {
            pmap,
            map: Spinlock::new(new_map),
        })
    }

    /// Clear the entire address-space.
    pub fn clear(&self) {
        unsafe {
            let mut map = self.map.lock();
            let mut tmp = InvasiveList::new();
            core::mem::swap(&mut tmp, &mut map);
            let mut fences = VmFenceSet::new();

            for entry in tmp.iter() {
                self.pmap
                    .unmap_multiple((*entry).range.start, (*entry).range.len());
                // Schedule VM fences for all affected pages.
                for vaddr in (*entry).range.clone() {
                    fences.add(Some(vaddr), None);
                }
            }
            vmfence::shootdown(&fences);

            // Drop all the removed mappings.
            while let Some(entry) = tmp.pop_front() {
                drop(Box::from_raw(entry));
            }
        }
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
    pub fn protect(&self, mut bounds: Range<usize>, prot_flags: u8) -> EResult<()> {
        if !is_canon_user_range(bounds.clone()) {
            return Err(Errno::EINVAL);
        }
        if bounds.start % PAGE_SIZE as usize != 0 {
            return Err(Errno::EINVAL);
        }
        bounds.end = bounds.end.div_ceil(PAGE_SIZE as usize) * PAGE_SIZE as usize;
        unsafe { self.0.protect(bounds, prot_flags) }
    }

    /// Unmap all pages within `bounds`.
    /// No changes are made on failure.
    /// Preserves the outer portion of mappings on the border of `bounds`.
    pub fn unmap(&self, bounds: Range<usize>) -> EResult<()> {
        if !is_canon_user_range(bounds.clone()) {
            return Err(Errno::EINVAL);
        }
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
        Ok(Self(self.0.fork()?))
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

impl Drop for KernelVmSpace {
    fn drop(&mut self) {
        panic!("Attempt to drop a KernelVmSpace");
    }
}
