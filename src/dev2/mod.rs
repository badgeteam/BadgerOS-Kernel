// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::sync::Arc;
use core::num::NonZeroU32;

#[cfg(feature = "acpi")]
pub mod acpi;
pub mod bus;
pub mod class;
pub mod driver;
#[cfg(feature = "dtb")]
pub mod dtb;
pub mod probe;
pub mod registry;

use class::{char::CharDevice, irqctl::IrqCtlDevice};

/// Base device struct; intended for use by implementers of [`Device`].
pub struct DeviceBase {
    /// ID assigned by the device registry.
    id: NonZeroU32,
}

impl DeviceBase {
    /// Create a new device base with a freshly allocated unique ID.
    pub fn new() -> Self {
        Self {
            id: registry::alloc_device_id(),
        }
    }

    /// ID assigned by the device registry.
    pub fn id(&self) -> NonZeroU32 {
        self.id
    }
}

impl Drop for DeviceBase {
    fn drop(&mut self) {
        registry::dealloc_device_id(self.id);
    }
}

/// List of device coercions.
macro_rules! dev_coercions {
    ($x:ident) => {
        $x!(char: CharDevice);
        $x!(irqctl: IrqCtlDevice);
    };
}

/// Helper for the ref coercion functions.
macro_rules! dev_ref_coercion {
    ($name:ident : $Type:ident) => {
        #[doc = concat!("Coerce this device into [`", stringify!($Type), "`]")]
        fn ${concat(as_, $name, _ref)} (&self) -> Option<&dyn $Type> { None }
    };
}

/// Helper for the [`Arc`] coercion functions.
macro_rules! dev_arc_coercion {
    ($name:ident : $Type:ident) => {
        #[doc = concat!("Coerce this device into [`", stringify!($Type), "`]")]
        pub fn ${concat(as_, $name)} (self: Arc<dyn Device>) -> Option<Arc<dyn $Type>> {
            unsafe {
                let ptr = Arc::into_raw(self);
                if let Some(coerced) = (*ptr).${concat(as_, $name, _ref)}() {
                    Some(Arc::from_raw(coerced))
                } else {
                    drop(Arc::from_raw(ptr));
                    None
                }
            }
        }
    };
}

/// An abstract device.
/// While some common logic is enforced for all devices, most of the logic depends on their specific types.
pub trait Device: Send + Sync + 'static {
    /// Get the base device struct.
    fn base(&self) -> &DeviceBase;

    /// Device interrupt handler; runs with interrupts disabled.
    /// Returns whether the interrupt was handled.
    fn interrupt(&self, id: u128) -> bool;

    dev_coercions!(dev_ref_coercion);
}

impl dyn Device {
    /// ID assigned by the device registry.
    /// The ID is unique to this device, even if not discoverable through the registry.
    pub fn id(&self) -> NonZeroU32 {
        self.base().id
    }

    dev_coercions!(dev_arc_coercion);
}
