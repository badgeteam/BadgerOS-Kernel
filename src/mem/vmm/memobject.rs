// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{
    bindings::error::EResult,
    mem::{pmm::PPN, vmm::VPN},
};

/// An object that can be mapped into a [`super::VMSpace`].
pub trait MemObject {
    /// Get the size in pages of the object.
    fn len(&self) -> VPN;

    /// Get a page from the object and increase its refcount.
    fn get(&self, page: VPN) -> EResult<PPN>;

    /// Release a reference to a page, decreasing its refcount.
    fn put(&self, page: VPN);

    /// Mark a page as being dirty.
    fn mark_dirty(&self, page: VPN);
}

pub struct ZeroFill;

impl MemObject for ZeroFill {
    fn len(&self) -> VPN {
        VPN::MAX
    }

    fn get(&self, page: VPN) -> EResult<PPN> {
        todo!()
    }

    fn put(&self, _page: VPN) {}

    fn mark_dirty(&self, _page: VPN) {}
}
