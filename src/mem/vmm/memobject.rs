// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::fmt::Debug;

use crate::{bindings::error::EResult, mem::pmm::PAddrr};

/// An object that can be mapped into a [`super::VMSpace`].
pub trait MemObject: Debug {
    /// Get the size in bytes of the object.
    /// Must be page-aligned.
    fn len(&self) -> usize;

    /// Whether to enable reference-counting for the pages from [`Self::get`].
    fn use_refcount(&self) -> bool;

    /// Try to get an existing page from the object.
    /// May spuriously return [`None`] even if the page is available.
    fn get(&self, offset: usize) -> Option<PAddrr>;

    /// Allocate a new page from the object.
    fn alloc(&self, offset: usize) -> EResult<PAddrr>;

    /// Mark a page as being dirty.
    fn mark_dirty(&self, offset: usize);
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
    fn len(&self) -> usize {
        self.len
    }

    fn use_refcount(&self) -> bool {
        false
    }

    fn get(&self, offset: usize) -> Option<PAddrr> {
        Some(self.paddr + offset)
    }

    fn alloc(&self, offset: usize) -> EResult<PAddrr> {
        Ok(self.paddr + offset)
    }

    fn mark_dirty(&self, _offset: usize) {}
}
