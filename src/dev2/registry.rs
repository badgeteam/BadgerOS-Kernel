// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    num::NonZeroU32,
    ptr::{DynMetadata, Pointee},
};

use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
#[cfg(feature = "dtb")]
use dtb::DtbNode;

use crate::{
    bindings::{error::EResult, log::LogLevel},
    dev2::probe,
    kernel::sync::mutex::{Mutex, SharedMutexGuard},
    util::id_alloc::IdAlloc,
};

use super::{Device, bus::Bus, driver::Driver};

/// ID allocator for devices.
static DEV_ID_ALLOC: Mutex<Option<IdAlloc>> = Mutex::new(None);
/// ID allocator for buses.
static BUS_ID_ALLOC: Mutex<Option<IdAlloc>> = Mutex::new(None);
/// ID allocator for bus reservations.
static RESV_ID_ALLOC: Mutex<Option<IdAlloc>> = Mutex::new(None);

/// Initialize the ID allocators.
pub fn init() {
    let mut dev_id_alloc = DEV_ID_ALLOC.unintr_lock();
    let mut bus_id_alloc = BUS_ID_ALLOC.unintr_lock();
    let mut resv_id_alloc = RESV_ID_ALLOC.unintr_lock();
    assert!(dev_id_alloc.is_none());
    assert!(bus_id_alloc.is_none());
    assert!(resv_id_alloc.is_none());
    *dev_id_alloc = Some(IdAlloc::new().expect("Out of memory"));
    *bus_id_alloc = Some(IdAlloc::new().expect("Out of memory"));
    *resv_id_alloc = Some(IdAlloc::new().expect("Out of memory"));
}

// region:devices

/// Map of all devices by ID.
static DEVICES: Mutex<BTreeMap<NonZeroU32, Arc<dyn Device>>> = Mutex::new(BTreeMap::new());

/// Allocate a new unique device ID.
/// Every device is given an ID even if it is never inserted into the registry.
pub(super) fn alloc_device_id() -> NonZeroU32 {
    DEV_ID_ALLOC
        .unintr_lock()
        .as_mut()
        .unwrap()
        .alloc()
        .unwrap()
}

/// Deallocate a device ID.
pub(super) fn dealloc_device_id(id: NonZeroU32) {
    debug_assert!(device_by_id(id).is_none());
    DEV_ID_ALLOC.unintr_lock().as_mut().unwrap().dealloc(id);
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

/// Allocate a new unique bus ID.
/// Every bus is given an ID even if it is never inserted into the registry.
pub(super) fn alloc_bus_id() -> NonZeroU32 {
    BUS_ID_ALLOC
        .unintr_lock()
        .as_mut()
        .unwrap()
        .alloc()
        .unwrap()
}

/// Deallocate a bus ID.
pub(super) fn dealloc_bus_id(id: NonZeroU32) {
    debug_assert!(bus_by_id(id).is_none());
    BUS_ID_ALLOC.unintr_lock().as_mut().unwrap().dealloc(id);
}

/// Allocate a new unique bus reservation ID.
pub(super) fn alloc_resv_id() -> NonZeroU32 {
    RESV_ID_ALLOC
        .unintr_lock()
        .as_mut()
        .unwrap()
        .alloc()
        .unwrap()
}

/// Deallocate a bus reservation ID.
pub(super) fn dealloc_resv_id(id: NonZeroU32) {
    RESV_ID_ALLOC.unintr_lock().as_mut().unwrap().dealloc(id);
}

/// Register a new bus, making it discoverable to driver probing.
pub fn register_bus(bus: Arc<dyn Bus>) -> EResult<()> {
    let id = bus.id();
    #[cfg(feature = "dtb")]
    if let Some(node) = bus.dtb_node() {
        let exist = BUS_BY_DTB.unintr_lock().insert(node, id);
        debug_assert!(exist.is_none());
    }
    let mut buses = BUSES.unintr_lock();
    let exist = buses.insert(id, bus.clone());
    debug_assert!(exist.is_none());
    probe::BUS_PROBE_LIST.unintr_lock().insert(bus);
    Ok(())
}

/// Remove a bus, preventing driver probing on it.
pub fn remove_bus(bus: &dyn Bus) {
    let mut buses = BUSES.unintr_lock();
    if buses.remove(&bus.id()).is_none() {
        logkf!(
            LogLevel::Warning,
            "Cannot remove unregistered bus {}",
            bus.id()
        );
        return;
    }
    probe::BUS_PROBE_LIST.unintr_lock().remove(bus);
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
            "Driver '{}' is already registered",
            driver.name()
        );
        return;
    }

    drivers.push(driver);
    if !BUSES.unintr_lock_shared().is_empty() {
        probe::DRIVER_PROBE_LIST.unintr_lock().push(driver);
    }
    logkf!(LogLevel::Info, "Register driver '{}'", driver.name());
}

/// Get all drivers.
pub fn all_drivers() -> SharedMutexGuard<'static, Vec<&'static dyn Driver>> {
    DRIVERS.unintr_lock_shared()
}

// endregion:drivers
