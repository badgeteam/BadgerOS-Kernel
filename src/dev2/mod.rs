// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::fmt::Debug;

use alloc::sync::Arc;

use crate::device::dtb::DtbNode;

pub mod addr;
pub mod family;

/// Device lifecycle stages.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DevLifecycle {
    /// Device is in the process of being registered.
    #[default]
    Inactive,
    /// Device is active, with or without a driver.
    Active,
    /// Device has been removed from the system.
    Removed,
}

/// The base device interface.
pub struct DeviceInfo {
    pub id: u32,
    /// Device that controls the bus this device is attached to.
    pub parent: Arc<DeviceInfo>,
    /// Device that receives interrupts from this device.
    pub irq_parent: Arc<DeviceInfo>,
    /// Associated DTB node.
    pub dtb_node: Option<&'static DtbNode>,
}

impl Debug for DeviceInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DeviceInfo")
            .field("id", &self.id)
            .field("parent", &self.parent)
            .field("irq_parent", &self.irq_parent)
            .field("dtb_node", &self.dtb_node.map(|x| x.name()))
            .finish()
    }
}
