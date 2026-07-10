// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::sync::Arc;
use core::{
    any::{Any, TypeId},
    cmp::Ordering,
    fmt::Display,
    num::NonZeroU32,
    ptr::{DynMetadata, NonNull, Pointee},
};

#[cfg(feature = "acpi")]
pub mod acpi;
pub mod bus;
pub mod class;
pub mod driver;
#[cfg(feature = "dtb")]
pub mod dtb;
pub mod probe;
pub mod registry;

/// Wrapper struct so that [`Device::get_trait_vtable()`] needs an `unsafe` to implement non-stub.
pub struct DevDynMetadata(NonNull<()>);

impl DevDynMetadata {
    /// # Safety
    /// This type is trusted by [`Device::get_trait_vtable()`], so it must be given the correct reference to take the vtable from.
    pub const unsafe fn new<T: ?Sized + Pointee<Metadata = DynMetadata<T>> + 'static>(
        ptr: &T,
    ) -> Self {
        unsafe { core::mem::transmute(core::ptr::metadata(ptr)) }
    }
}

/// Helper macro to implement [`Device::get_trait_vtable()`].
/// Place this inside the `impl Device for T` block.
#[macro_export]
macro_rules! device_get_trait_vtable {
    ($($traits: path), *) => {
        #[allow(unused)]
        fn get_trait_vtable(&self, trait_: core::any::TypeId) -> Option<crate::dev2::DevDynMetadata> {
            $(
                if core::any::TypeId::of::<dyn $traits>() == trait_ {
                    unsafe {
                        return Some(crate::dev2::DevDynMetadata::new::<dyn $traits>(self));
                    }
                }
            )*
            None
        }
    };
}

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

// TODO: Callback for removal/deregister, etc.
/// An abstract device.
/// While some common logic is enforced for all devices, most of the logic depends on their specific types.
pub trait Device: Display + Any + Send + Sync + 'static {
    /// Get the base device struct.
    fn base(&self) -> &DeviceBase;

    /// Device interrupt handler; runs with interrupts disabled.
    /// Returns whether the interrupt was handled.
    fn interrupt(&self, id: u128) -> bool;

    /// Test whether this device implements a trait and get its metadata if so.
    /// Should not be used directly.
    fn get_trait_vtable(&self, _trait: TypeId) -> Option<DevDynMetadata> {
        None
    }
}

impl dyn Device {
    /// ID assigned by the device registry.
    /// The ID is unique to this device, even if not discoverable through the registry.
    pub fn id(&self) -> NonZeroU32 {
        self.base().id
    }

    /// Try to get as given trait.
    pub fn try_as_ref<T: ?Sized + Pointee<Metadata = DynMetadata<T>> + 'static>(
        &self,
    ) -> Option<&T> {
        let meta = self.get_trait_vtable(TypeId::of::<T>())?;
        unsafe {
            let ptr: *const T = core::ptr::from_raw_parts(
                self as *const dyn Device as *const (),
                core::mem::transmute(meta),
            );
            Some(&*ptr)
        }
    }

    /// Try to get as given trait.
    pub fn try_as_arc<T: ?Sized + Pointee<Metadata = DynMetadata<T>> + 'static>(
        self: Arc<Self>,
    ) -> Option<Arc<T>> {
        let meta = self.get_trait_vtable(TypeId::of::<T>())?;
        unsafe {
            let this = Arc::into_raw(self);
            let ptr: *const T = core::ptr::from_raw_parts(
                this as *const dyn Device as *const (),
                core::mem::transmute(meta),
            );
            Some(Arc::from_raw(ptr))
        }
    }
}

impl PartialEq for dyn Device {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}
impl Eq for dyn Device {}

impl PartialOrd for dyn Device {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for dyn Device {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id().cmp(&other.id())
    }
}

/// Initialize the device subsystem.
pub unsafe fn init() {
    registry::init();

    #[cfg(feature = "dtb")]
    {
        use crate::boot;

        let fdt_ptr = boot::protocol::get_fdt_ptr();
        if !fdt_ptr.is_null() {
            unsafe { dtb::init(fdt_ptr as _) };
        }
    }

    probe::start_thread();
}
