// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::any::Any;

use super::DeviceInfo;

pub mod pci;

/// The base device driver interface.
pub trait Driver: Sync {
    /// An interrupt was received thay may belong to this device.
    /// Returns whether the interrupt was serviced by this driver.
    fn interrupt(&self, dev: &DeviceInfo, irq: usize) -> bool;

    /// This device was removed from the system.
    /// An abruptly removed device may be unreachable.
    fn removed(&self, dev: &DeviceInfo, abrubt: bool) -> bool;
}

/// The base device usage interface.
pub trait Interface: Any {}
