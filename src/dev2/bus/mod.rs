// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{any::Any, num::NonZeroU32};

use alloc::sync::{Arc, Weak};
use dtb::DtbNode;

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    dev2::Device,
    kernel::sync::mutex::Mutex,
};

use super::registry;

pub mod soc;

/// Used to provide a concrete type for [`Weak::new`] to work for a `Weak<dyn Device>`.
/// You shouldn't instantiate a proper device with this, so all functions are `unreachable!()`.
struct DummyDevice;

impl Device for DummyDevice {
    fn base(&self) -> &super::DeviceBase {
        unreachable!()
    }

    fn interrupt(&self, _id: u128) -> bool {
        unreachable!()
    }

    fn get_trait_vtable(&self, _trait: core::any::TypeId) -> Option<super::DevDynMetadata> {
        unreachable!()
    }
}

/// Base bus struct; intended for use by implementers of [`Bus`].
pub struct BusBase {
    /// ID assigned by the device registry.
    id: NonZeroU32,
    /// For what device this bus is currently in use, if any.
    reservation: Mutex<Weak<dyn Device>>,
}

impl BusBase {
    /// Create a new device base with a freshly allocated unique ID.
    pub fn new() -> Self {
        Self {
            id: registry::alloc_bus_id(),
            reservation: Mutex::new(Weak::<DummyDevice>::new()),
        }
    }

    /// ID assigned by the device registry.
    pub fn id(&self) -> NonZeroU32 {
        self.id
    }

    /// Try to claim this bus for a given device.
    /// If the device becomes unreferenced, the bus is released automatically.
    ///
    /// Using a bus without claiming it will usually work, but multiple drivers contending for one bus
    /// will likely cause unintended behaviour.
    pub fn claim(&self, device: Weak<dyn Device>) -> EResult<()> {
        if device.strong_count() == 0 {
            logkf!(LogLevel::Warning, "Bus::claim with empty device");
            return Err(Errno::ENOENT);
        }

        let mut guard = self.reservation.unintr_lock();
        if guard.strong_count() != 0 {
            // Bus is still in use.
            return Err(Errno::EADDRNOTAVAIL);
        }
        // Bus is available.
        *guard = device;

        Ok(())
    }

    /// Release the claim on this bus.
    pub fn release(&self, device: &dyn Device) {
        let mut guard = self.reservation.unintr_lock();
        if !core::ptr::addr_eq(guard.as_ptr(), device) {
            logkf!(
                LogLevel::Warning,
                "Bus::release by device that did not own it"
            );
            return;
        }
        *guard = Weak::<DummyDevice>::new();
    }

    /// Check which device currently owns this bus.
    pub fn owner(&self) -> Option<Arc<dyn Device>> {
        self.reservation.unintr_lock_shared().upgrade()
    }
}

impl Drop for BusBase {
    fn drop(&mut self) {
        registry::dealloc_bus_id(self.id);
    }
}

/// A bus is how a driver communicates with devices.
/// Buses may exist on their own (e.g. MMIO) or as part of another device (e.g. AHCI ports).
/// Most of the logic for buses depends on their specific types, this trait serves mostly to register buses.
/// A single bus usually supports multiple devices.
pub trait Bus: Any + Send + Sync + 'static {
    /// Get the base bus struct.
    fn base(&self) -> &BusBase;

    /// Which device provided this bus, if any.
    fn parent_device(&self) -> Option<Arc<dyn Device>>;

    /// Install the handler for an interrupt.
    ///
    /// # Safety
    /// The caller promises to remove the handler with [`Bus::uninstall_irq`] before it becomes invalid.
    ///
    /// The caller promises that the handler is a valid [`Device`] object.
    unsafe fn install_irq(&self, irq_id: u128, device: *const dyn Device) -> EResult<()>;

    /// Remove the handler for an interrupt.
    ///
    /// # Safety
    /// The caller promises that the handler is a valid [`Device`] object.
    unsafe fn uninstall_irq(&self, irq_id: u128, device: *const dyn Device);

    /// Associated DTB node, if any.
    fn dtb_node(&self) -> Option<&'static DtbNode>;

    /// ID assigned by the device registry.
    /// The ID is unique to this bus, even if not discoverable through the registry.
    fn id(&self) -> NonZeroU32 {
        self.base().id
    }

    /// Try to claim this bus for a given device.
    /// If the device becomes unreferenced, the bus is released automatically.
    ///
    /// Using a bus without claiming it will usually work, but multiple drivers contending for one bus
    /// will likely cause unintended behaviour.
    fn claim(&self, device: Weak<dyn Device>) -> EResult<()> {
        self.base().claim(device)
    }

    /// Release the claim on this bus.
    fn release(&self, device: &dyn Device) {
        self.base().release(device);
    }

    /// Check which device currently owns this bus.
    fn owner(&self) -> Option<Arc<dyn Device>> {
        self.base().owner()
    }
}
