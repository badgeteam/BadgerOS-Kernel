// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::bindings::error::EResult;

use super::{PPN, PageUsage, page_alloc, page_free, page_struct};

/// Helper struct for making page allocation cleanup easier.
#[repr(C)]
pub struct PhysPtr {
    ppn: PPN,
    order: u8,
}

impl PhysPtr {
    /// Try to allocate a new block of memory.
    pub fn new(order: u8, usage: PageUsage) -> EResult<Self> {
        Ok(Self {
            ppn: unsafe { page_alloc(order, usage) }?,
            order,
        })
    }

    /// Create from page number and order that was allocated earlier.
    pub unsafe fn from_raw_parts(ppn: PPN, order: u8) -> Self {
        Self { ppn, order }
    }

    /// Create from page number that was allocated earlier, reading the order from the page structs.
    pub unsafe fn from_raw_ppn(ppn: PPN) -> Self {
        let order = unsafe { (*page_struct(ppn)).order() };
        Self {
            ppn: ppn >> order << order,
            order,
        }
    }

    /// Log-base-2 of the allocation's size.
    pub fn order(&self) -> u8 {
        self.order
    }

    /// Physical page number of the start of the allocation.
    pub fn ppn(&self) -> PPN {
        self.ppn
    }

    /// Decompose into page number and order without freeing.
    #[must_use]
    pub fn into_raw_parts(self) -> (PPN, u8) {
        let tmp = (self.ppn, self.order);
        core::mem::forget(self);
        tmp
    }
}

impl Drop for PhysPtr {
    fn drop(&mut self) {
        unsafe { page_free(self.ppn, self.order) };
    }
}
