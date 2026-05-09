// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ops::Range;

use alloc::{boxed::Box, sync::Arc};

use crate::{
    dev2::{bus::Bus, device::Device},
    device::dtb::DtbNode,
    mem::pmm::PAddrr,
};

/// Memory-mapped I/O bus.
pub struct MmioBus {
    /// Associated DTB node, if any.
    dtb_node: Option<&'static DtbNode>,
    /// Physical addresses in this MMIO bus.
    paddr: Box<[Range<PAddrr>]>,
}

impl MmioBus {
    pub fn new(dtb_node: Option<&'static DtbNode>, paddr: Box<[Range<PAddrr>]>) -> Self {
        assert!(paddr.len() >= 1);
        Self { dtb_node, paddr }
    }
}

impl Bus for MmioBus {
    fn parent_device(&self) -> Option<Arc<dyn Device>> {
        None
    }

    /// Associated DTB node, if any.
    fn dtb_node(&self) -> Option<&'static DtbNode> {
        self.dtb_node
    }
}
