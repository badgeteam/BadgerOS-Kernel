// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ops::Range;

use crate::{
    bindings::error::EResult,
    mem::{pmm::PPN, vmm::VPN},
};

/// Pager operations referenced by [`super::mobj::MemObject`].
pub trait Pager {
    /// Get or read a page from this object.
    fn get_page(&mut self, offset: VPN) -> EResult<PPN>;
    /// Write back a range of pages.
    fn sync_pages(&mut self, offset: Range<VPN>) -> EResult<()>;
    /// Return a page that is no longer referenced.
    fn drop_page(&mut self, offset: VPN);
}

struct ZeroFillPager;

impl Pager for ZeroFillPager {
    fn get_page(&mut self, _offset: VPN) -> EResult<PPN> {
        todo!("Return PPN of the zeroes page")
    }

    fn sync_pages(&mut self, _offset: Range<VPN>) -> EResult<()> {
        unreachable!("sync_pages on ZeroFillPager");
    }

    fn drop_page(&mut self, _offset: VPN) {}
}
