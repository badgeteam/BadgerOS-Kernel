// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::sync::Arc;
use dtb::DtbNode;

use crate::{bindings::error::EResult, dev2::Device};

pub mod mmio;

/// A bus is how a driver communicates with devices.
/// Buses may exist on their own (e.g. MMIO) or as part of another device (e.g. AHCI ports).
/// Most of the logic for buses depends on their specific types, this trait serves mostly to register buses.
/// A single bus usually supports multiple devices.
pub trait Bus: Send + Sync + 'static {
    /// Which device provided this bus, if any.
    fn parent_device(&self) -> Option<Arc<dyn Device>>;

    /// Install the handler for an interrupt.
    ///
    /// # Safety
    /// The caller promises to remove the handler with [`Bus::remove_irq`] before it becomes invalid.
    unsafe fn install_irq(&self, irq_id: u128, device: *const dyn Device) -> EResult<()>;

    /// Remove the handler for an interrupt.
    ///
    /// # Safety
    /// The caller promises that the handler is a valid [`Device`] object.
    unsafe fn uninstall_irq(&self, irq_id: u128, device: *const dyn Device);

    /// Associated DTB node, if any.
    fn dtb_node(&self) -> Option<&'static DtbNode>;
}
