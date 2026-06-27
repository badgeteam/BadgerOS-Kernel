// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    num::NonZeroU32,
    sync::atomic::{AtomicU32, Ordering},
};

use alloc::{collections::btree_map::BTreeMap, sync::Arc};

use crate::{bindings::error::EResult, kernel::sync::mutex::Mutex};

use super::Device;

/// The actual storage of the registry.
static DEVICES: Mutex<BTreeMap<NonZeroU32, Arc<dyn Device>>> = Mutex::new(BTreeMap::new());

/// Next device ID to hand out.
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// Allocate a new unique device ID.
/// Every device is given an ID even if it is never inserted into the registry.
pub fn alloc_id() -> NonZeroU32 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    NonZeroU32::new(id).expect("Device ID counter overflow")
}

/// Register a new device.
pub fn register_device(device: Arc<dyn Device>) -> EResult<()> {
    let id = device.id();
    DEVICES.lock()?.insert(id, device);
    Ok(())
}

/// Remove a device from the registry if it exists.
pub fn remove_device(device: &dyn Device) -> EResult<()> {
    DEVICES.lock()?.remove(&device.id());
    Ok(())
}

/// Get a device by ID.
pub fn by_id(id: NonZeroU32) -> Option<Arc<dyn Device>> {
    DEVICES.lock().ok()?.get(&id).cloned()
}
