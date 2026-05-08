// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{any::Any, num::NonZeroU32};

use alloc::sync::Arc;

use crate::bindings::error::EResult;

/// A bus is how a driver communicates with devices.
/// Buses may exist on their own (e.g. MMIO) or as part of another device (e.g. AHCI ports).
/// Most of the logic for buses depends on their specific types, this trait serves mostly to register buses.
/// A single bus usually supports multiple devices.
pub trait Bus: Any + Send + Sync {
    /// Which device provided this bus, if any.
    fn parent_device(&self) -> Option<Arc<dyn Device>>;
}

/// Base device metadata struct; intended for use by implementers of [`Device`].
pub struct DeviceMeta {
    /// Bus to which this device is connected.
    bus: Arc<dyn Bus>,
    /// ID assigned by the device registry.
    id: NonZeroU32,
}

/// An abstract device.
/// While some common logic is enforced for all devices, most of the logic depends on their specific types.
pub trait Device: Any + Send + Sync {
    /// Get the base device struct.
    fn base(&self) -> &DeviceMeta;
}

impl dyn Device + '_ {
    /// Register a new device; it will be assigned an ID and become discoverable.
    pub fn register(device: Arc<dyn Device>) -> EResult<()> {
        todo!()
    }

    /// ID assigned by the device registry.
    /// The ID is unique to this device, even if not discoverable through the registry.
    pub fn id(&self) -> NonZeroU32 {
        self.base().id
    }
}
