// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ops::Range;

use alloc::{boxed::Box, collections::linked_list::LinkedList, sync::Arc};

use crate::{kernel::sync::spinlock::Spinlock, mem::pmm::Page};

use super::*;

/// Mapping is shared (not CoW'ed on fork).
pub const SHARED: u32 = 0x01;
/// Mapping is private (CoW'ed on fork).
pub const PRIVATE: u32 = 0x02;
/// Replace the mapping at the given address if it exists.
pub const FIXED: u32 = 0x10;

/// A region of anonymous memory.
struct Anon {
    /// Current page for this anon.
    page: *const Page,
}

#[derive(Clone)]
struct AMap {
    /// Offset from the parent [`MapEntry`].
    offset: VPN,
    /// Resident pages of this region.
    pages: Option<Box<[Option<Arc<Anon>>]>>,
}

/// Entry in the linked list in [`Map`].
#[derive(Clone)]
struct MapEntry {
    /// Region start and end.
    range: Range<VPN>,
    /// Region protection flags.
    prot: u8,
    /// Region mapping flags.
    map: u8,
    /// Anonymous memory overlay.
    amap: Option<Arc<AMap>>,
}

/// Virtual address-space map.
pub struct Map {
    regions: LinkedList<Spinlock<AMap>>,
}
