// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::sync::Arc;

use crate::{
    bindings::error::EResult,
    dev2::{
        Device,
        bus::{Bus, BusResv},
    },
};

pub trait Driver: Send + Sync + 'static {
    /// Human-readable name.
    fn name(&self) -> &str;

    /// Test whether a bus matches this driver.
    fn match_(&self, bus: &dyn Bus) -> bool;

    /// Try to probe the device on a given bus.
    ///
    /// # Safety
    /// It is unsafe to mislead a driver as to what the device is.
    /// In practice, however, there is little we can do more than hoping
    /// the DTB/ACPI tables were correct.
    unsafe fn probe(&self, bus: BusResv<dyn Bus>) -> EResult<Arc<dyn Device>>;
}
