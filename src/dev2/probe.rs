// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::sync::Arc;

use crate::bindings::log::LogLevel;

use super::{
    bus::Bus,
    driver::Driver,
    registry::{self, all_drivers},
};

/// Match a driver for and probe on a bus.
/// If a match is found, returns the driver that matched it.
pub unsafe fn probe_bus(bus: Arc<dyn Bus>) -> Option<&'static dyn Driver> {
    for &driver in &*all_drivers() {
        if driver.match_(&*bus) {
            match try { registry::register_device(unsafe { driver.probe(bus.clone())? })? } {
                Ok(()) => {
                    return Some(driver);
                }
                Err(x) => logkf!(
                    LogLevel::Error,
                    "Failed to probe driver for bus {}: {}",
                    bus.id(),
                    x
                ),
            }
        }
    }
    None
}
