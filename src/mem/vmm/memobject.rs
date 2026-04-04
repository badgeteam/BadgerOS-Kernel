// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{
    bindings::error::EResult,
    mem::{
        pmm::PPN,
        vmm::{PAGE_OF_ZEROES, VPN},
    },
};

/// An object that can be mapped into a [`super::VMSpace`].
pub trait MemObject {
    /// Get the size in pages of the object.
    fn len(&self) -> VPN;

    /// Whether to enable reference-counting for the pages from [`Self::get`].
    fn use_refcount(&self) -> bool;

    /// Get a page from the object.
    fn get(&self, page: VPN) -> EResult<PPN>;

    /// Mark a page as being dirty.
    fn mark_dirty(&self, page: VPN);
}

pub struct ZeroFill;

impl MemObject for ZeroFill {
    fn len(&self) -> VPN {
        VPN::MAX
    }

    fn use_refcount(&self) -> bool {
        false
    }

    fn get(&self, _page: VPN) -> EResult<PPN> {
        unsafe { Ok(PAGE_OF_ZEROES) }
    }

    fn mark_dirty(&self, _page: VPN) {}
}
