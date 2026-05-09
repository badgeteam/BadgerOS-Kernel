// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::any::Any;

use alloc::sync::Arc;

use crate::{dev2::device::Device, device::dtb::DtbNode};

pub mod mmio;

/// A bus is how a driver communicates with devices.
/// Buses may exist on their own (e.g. MMIO) or as part of another device (e.g. AHCI ports).
/// Most of the logic for buses depends on their specific types, this trait serves mostly to register buses.
/// A single bus usually supports multiple devices.
pub trait Bus: Any + Send + Sync {
    /// Which device provided this bus, if any.
    fn parent_device(&self) -> Option<Arc<dyn Device>>;

    /// Associated DTB node, if any.
    fn dtb_node(&self) -> Option<&'static DtbNode>;
}
