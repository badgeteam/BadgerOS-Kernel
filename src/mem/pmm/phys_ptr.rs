// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::bindings::error::EResult;

use super::{PAddrr, PageUsage, page_alloc, page_free};

/// Helper struct for making page allocation cleanup easier.
#[repr(C)]
pub struct PhysPtr {
    paddr: PAddrr,
    order: u8,
}

impl PhysPtr {
    /// Try to allocate a new block of memory.
    pub fn new(order: u8, usage: PageUsage) -> EResult<Self> {
        Ok(Self {
            paddr: unsafe { page_alloc(order, usage) }?,
            order,
        })
    }

    /// Create from page number and order that was allocated earlier.
    pub unsafe fn from_raw_parts(paddr: PAddrr, order: u8) -> Self {
        Self { paddr, order }
    }

    /// Log-base-2 of the allocation's size.
    pub fn order(&self) -> u8 {
        self.order
    }

    /// Physical address of the start of the allocation.
    pub fn paddr(&self) -> PAddrr {
        self.paddr
    }

    /// Decompose into page number and order without freeing.
    #[must_use]
    pub fn into_raw_parts(self) -> (PAddrr, u8) {
        let tmp = (self.paddr, self.order);
        core::mem::forget(self);
        tmp
    }
}

impl Drop for PhysPtr {
    fn drop(&mut self) {
        unsafe { page_free(self.paddr, self.order) };
    }
}
