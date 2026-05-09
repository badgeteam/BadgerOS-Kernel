// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{any::Any, num::NonZeroU32};

pub mod ns16550a;

/// Base device metadata struct; intended for use by implementers of [`Device`].
pub struct DeviceMeta {
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
    /// ID assigned by the device registry.
    /// The ID is unique to this device, even if not discoverable through the registry.
    pub fn id(&self) -> NonZeroU32 {
        self.base().id
    }
}
