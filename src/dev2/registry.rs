// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    num::NonZeroU32,
    ptr::{DynMetadata, Pointee},
    sync::atomic::{AtomicU32, Ordering},
};

use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
#[cfg(feature = "dtb")]
use dtb::DtbNode;

use crate::{
    bindings::{error::EResult, log::LogLevel},
    kernel::sync::mutex::{Mutex, SharedMutexGuard},
};

use super::{Device, bus::Bus, driver::Driver};

// region:devices

/// Map of all devices by ID.
static DEVICES: Mutex<BTreeMap<NonZeroU32, Arc<dyn Device>>> = Mutex::new(BTreeMap::new());

/// Next device ID to hand out.
static NEXT_DEV_ID: AtomicU32 = AtomicU32::new(1);

/// Allocate a new unique device ID.
/// Every device is given an ID even if it is never inserted into the registry.
pub(super) fn alloc_device_id() -> NonZeroU32 {
    let id = NEXT_DEV_ID.fetch_add(1, Ordering::Relaxed);
    NonZeroU32::new(id).expect("Device ID counter overflow")
}

/// Deallocate a device id.
pub(super) fn dealloc_device_id(id: NonZeroU32) {
    debug_assert!(device_by_id(id).is_none());
    // TODO.
}

/// Register a new device.
pub fn register_device(device: Arc<dyn Device>) -> EResult<()> {
    let id = device.id();
    DEVICES.unintr_lock().insert(id, device);
    Ok(())
}

/// Remove a device from the registry if it exists.
pub fn remove_device(device: &dyn Device) {
    if DEVICES.unintr_lock().remove(&device.id()).is_none() {
        logkf!(
            LogLevel::Warning,
            "Cannot remove unregistered device {}",
            device.id()
        );
    }
}

/// Get a device by ID.
pub fn device_by_id(id: NonZeroU32) -> Option<Arc<dyn Device>> {
    DEVICES.unintr_lock_shared().get(&id).cloned()
}

/// Get all devices.
pub fn devices() -> SharedMutexGuard<'static, BTreeMap<NonZeroU32, Arc<dyn Device>>> {
    DEVICES.unintr_lock_shared()
}

/// Filter devices by trait implementation.
pub fn devices_by_trait<T: ?Sized + Pointee<Metadata = DynMetadata<T>> + 'static>()
-> EResult<Vec<Arc<T>>> {
    let devices = DEVICES.unintr_lock_shared();
    let mut res = Vec::new();

    for device in devices.values() {
        if let Some(x) = device.clone().try_as_arc() {
            res.try_reserve(1)?;
            res.push(x);
        }
    }

    Ok(res)
}

// endregion:devices

// region:buses

/// Set of all buses.
static BUSES: Mutex<BTreeMap<NonZeroU32, Arc<dyn Bus>>> = Mutex::new(BTreeMap::new());

/// Bus ID by DTB node.
#[cfg(feature = "dtb")]
static BUS_BY_DTB: Mutex<BTreeMap<*const DtbNode, NonZeroU32>> = Mutex::new(BTreeMap::new());

/// Next bus ID to hand out.
static NEXT_BUS_ID: AtomicU32 = AtomicU32::new(1);

/// Allocate a new unique bus ID.
/// Every bus is given an ID even if it is never inserted into the registry.
pub(super) fn alloc_bus_id() -> NonZeroU32 {
    let id = NEXT_BUS_ID.fetch_add(1, Ordering::Relaxed);
    NonZeroU32::new(id).expect("Bus ID counter overflow")
}

/// Deallocate a device id.
pub(super) fn dealloc_bus_id(id: NonZeroU32) {
    debug_assert!(bus_by_id(id).is_none());
    // TODO.
}

/// Register a new bus, making it discoverable to driver probing.
pub fn register_bus(bus: Arc<dyn Bus>) -> EResult<()> {
    let id = bus.id();
    #[cfg(feature = "dtb")]
    if let Some(node) = bus.dtb_node() {
        let exist = BUS_BY_DTB.unintr_lock().insert(node, id);
        debug_assert!(exist.is_none());
    }
    let exist = BUSES.unintr_lock().insert(id, bus);
    debug_assert!(exist.is_none());
    Ok(())
}

/// Remove a bus, preventing driver probing on it.
pub fn remove_bus(bus: &dyn Bus) {
    if BUSES.unintr_lock().remove(&bus.id()).is_none() {
        logkf!(
            LogLevel::Warning,
            "Cannot remove unregistered bus {}",
            bus.id()
        );
        return;
    }
    #[cfg(feature = "dtb")]
    if let Some(node) = bus.dtb_node() {
        BUS_BY_DTB.unintr_lock().remove(&(node as *const DtbNode));
    }
}

/// Get a bus by ID.
pub fn bus_by_id(id: NonZeroU32) -> Option<Arc<dyn Bus>> {
    BUSES.unintr_lock_shared().get(&id).cloned()
}

/// Get a bus by DTB node.
pub fn bus_by_node(node: &'static DtbNode) -> Option<Arc<dyn Bus>> {
    let id = *BUS_BY_DTB
        .unintr_lock_shared()
        .get(&(node as *const DtbNode))?;
    bus_by_id(id)
}

/// Get all buses.
pub fn buses() -> SharedMutexGuard<'static, BTreeMap<NonZeroU32, Arc<dyn Bus>>> {
    BUSES.unintr_lock_shared()
}

/// Filter all buses by concete type.
pub fn buses_by_type<T: Bus>() -> EResult<Vec<Arc<T>>> {
    let buses = BUSES.unintr_lock_shared();
    let mut res = Vec::new();

    for bus in buses.values() {
        if let Ok(x) = Arc::downcast(bus.clone()) {
            res.try_reserve(1)?;
            res.push(x);
        }
    }

    Ok(res)
}

// endregion:buses

// region:drivers

/// Set of all drivers.
static DRIVERS: Mutex<Vec<&'static dyn Driver>> = Mutex::new(Vec::new());

/// Register a new driver.
pub fn register_driver(driver: &'static dyn Driver) {
    let mut drivers = DRIVERS.unintr_lock();

    if drivers.iter().any(|x| core::ptr::addr_eq(x, driver)) {
        logkf!(
            LogLevel::Warning,
            "Driver \"{}\" is already registered",
            driver.name()
        );
        return;
    }

    drivers.push(driver);
    logkf!(LogLevel::Info, "Register driver \"{}\"", driver.name());
}

/// Get all drivers.
pub fn all_drivers() -> SharedMutexGuard<'static, Vec<&'static dyn Driver>> {
    DRIVERS.unintr_lock_shared()
}

// endregion:drivers
