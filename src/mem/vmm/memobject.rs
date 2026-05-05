// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{fmt::Debug, num::NonZeroUsize, sync::atomic::Ordering};

use crate::{
    bindings::{error::EResult, log::LogLevel},
    config::PAGE_SIZE,
    mem::pmm::{self, PAddrr},
};

use super::map::{MapEntry, VmSpaceInner};

/// A page that can be memory-mapped.
pub struct MappablePage(NonZeroUsize);

impl MappablePage {
    pub unsafe fn new(paddr: usize, refcounted: bool, writable: bool, tracks_dirty: bool) -> Self {
        assert!(paddr != 0);
        assert!(paddr % PAGE_SIZE as usize == 0);
        // SAFETY: Already checked for zero with the assert above.
        Self(unsafe {
            NonZeroUsize::new_unchecked(
                paddr + tracks_dirty as usize * 4 + refcounted as usize * 2 + writable as usize,
            )
        })
    }

    pub fn clear_writable(&mut self) {
        let tmp = self.0.get() & !1usize;
        // SAFETY: Since the page cannot be null and is page-aligned, this can't make a zero value.
        self.0 = unsafe { NonZeroUsize::new_unchecked(tmp) };
    }

    pub const fn paddr(&self) -> PAddrr {
        self.0.get() & (PAGE_SIZE as usize).wrapping_neg()
    }

    pub const fn writable(&self) -> bool {
        (self.0.get() & 1) != 0
    }

    pub const fn refcounted(&self) -> bool {
        (self.0.get() & 2) != 0
    }

    pub const fn tracks_dirty(&self) -> bool {
        (self.0.get() & 4) != 0
    }

    pub const fn into_paddr(self) -> PAddrr {
        // SAFETY: Transparent representation allows transmutation into inner type.
        (unsafe { core::mem::transmute::<_, usize>(self) }) & (PAGE_SIZE as usize).wrapping_neg()
    }
}

impl Drop for MappablePage {
    fn drop(&mut self) {
        if self.refcounted() {
            unsafe {
                let meta = pmm::page_struct_base(self.paddr());
                let rc = (*meta.0).refcount.fetch_sub(1, Ordering::Relaxed);
                if rc == 0 {
                    logkf!(
                        LogLevel::Warning,
                        "Refcount underflow for physical page at 0x{:x}",
                        self.paddr() * PAGE_SIZE as usize
                    );
                }
            }
        }
    }
}

/// An object that can be mapped into a [`super::map::VmSpace`].
pub trait MemObject: Debug {
    /// Get the size in bytes of the object.
    /// Must be page-aligned.
    fn len(&self) -> u64;

    /// Called when a new mapping entry is made with this memory object.
    /// The [`VmSpaceInner`] promises that [`MemObject::on_unmapped`] is called before the pointers become invalid.
    fn on_mapped(
        &self,
        _denywrite: bool,
        _vmspace: *const VmSpaceInner,
        _range: *const MapEntry,
    ) -> EResult<()> {
        Ok(())
    }

    /// Called when a new mapping entry is made with this memory object.
    fn on_unmapped(
        &self,
        _denywrite: bool,
        _vmspace: *const VmSpaceInner,
        _range: *const MapEntry,
    ) {
    }

    /// Try to get an existing page from the object.
    /// May spuriously return [`None`] even if the page is available.
    fn get(&self, offset: u64) -> Option<MappablePage>;

    /// Allocate a new page from the object.
    fn alloc(&self, offset: u64) -> EResult<MappablePage>;

    /// Mark a page as being dirty.
    fn mark_dirty(&self, offset: u64);

    /// Whether this object tracks dirtyness.
    fn tracks_dirty(&self) -> bool;

    /// Returns true if any page is currently dirty.
    fn has_dirty_pages(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub struct RawMemory {
    paddr: PAddrr,
    len: usize,
}

impl RawMemory {
    pub const unsafe fn new(paddr: PAddrr, len: usize) -> Self {
        Self { paddr, len }
    }
}

impl MemObject for RawMemory {
    fn len(&self) -> u64 {
        self.len as u64
    }

    fn get(&self, offset: u64) -> Option<MappablePage> {
        // SAFETY: The creator of this object guaranteed that the memory is valid.
        Some(unsafe { MappablePage::new(self.paddr + offset as usize, false, true, false) })
    }

    fn alloc(&self, offset: u64) -> EResult<MappablePage> {
        // SAFETY: The creator of this object guaranteed that the memory is valid.
        Ok(unsafe { MappablePage::new(self.paddr + offset as usize, false, true, false) })
    }

    fn mark_dirty(&self, _offset: u64) {}

    fn tracks_dirty(&self) -> bool {
        false
    }
}
