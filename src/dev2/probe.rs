// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::{boxed::Box, collections::btree_set::BTreeSet, sync::Arc, vec::Vec};

use crate::{
    bindings::{log::LogLevel, raw::timestamp_us_t},
    kernel::{
        sched::Thread,
        sync::{mutex::Mutex, waitlist::Waitlist},
    },
};

use super::{
    bus::Bus,
    driver::Driver,
    registry::{self, all_drivers},
};

/// Waitlist that the driver probe thread waits on for events.
pub(super) static WAITLIST: Waitlist = Waitlist::new();
/// Buses that have been registered but not yet had drivers probed.
pub(super) static BUS_PROBE_LIST: Mutex<BTreeSet<Arc<dyn Bus>>> = Mutex::new(BTreeSet::new());
/// Drivers that have been registered but not yet probed against existing buses without devices.
pub(super) static DRIVER_PROBE_LIST: Mutex<Vec<&'static dyn Driver>> = Mutex::new(Vec::new());

fn probe_driver_impl(driver: &'static dyn Driver) {
    let driverless: Box<[_]> = registry::buses()
        .values()
        .filter(|&x| x.owner().is_none())
        .map(Arc::clone)
        .collect();

    for bus in driverless.into_iter() {
        if driver.match_(&*bus) {
            let res = try {
                let device = unsafe { driver.probe(bus.clone())? };
                registry::register_device(device)?;
            };
            match res {
                Ok(()) => {
                    logkf!(
                        LogLevel::Info,
                        "Probed driver '{}' for {}",
                        driver.name(),
                        bus
                    );
                }
                Err(x) => {
                    logkf!(
                        LogLevel::Error,
                        "Failed to probe driver '{}' for {}: {}",
                        driver.name(),
                        bus,
                        x
                    );
                }
            }
        }
    }
}

fn probe_bus_impl(bus: Arc<dyn Bus>) {
    for &driver in &*all_drivers() {
        if driver.match_(&*bus) {
            match try { registry::register_device(unsafe { driver.probe(bus.clone())? })? } {
                Ok(()) => logkf!(
                    LogLevel::Info,
                    "Probed driver '{}' for {}",
                    driver.name(),
                    bus
                ),
                Err(x) => logkf!(
                    LogLevel::Error,
                    "Failed to probe driver '{}' for {}: {}",
                    driver.name(),
                    bus,
                    x
                ),
            }
        }
    }
}

/// Match a driver for and probe on a bus.
/// If a match is found, returns the driver that matched it.
pub unsafe fn probe_bus(bus: Arc<dyn Bus>) {
    BUS_PROBE_LIST.unintr_lock().remove(&bus);
    probe_bus_impl(bus)
}

/// Driver probing loop.
fn probe_loop() {
    WAITLIST.unintr_block(timestamp_us_t::MAX, || {
        BUS_PROBE_LIST.unintr_lock_shared().is_empty()
            && DRIVER_PROBE_LIST.unintr_lock_shared().is_empty()
    });

    let mut buses = BTreeSet::new();
    core::mem::swap(&mut buses, &mut BUS_PROBE_LIST.unintr_lock());
    for bus in buses.into_iter() {
        probe_bus_impl(bus);
    }

    let mut drivers = Vec::new();
    core::mem::swap(&mut drivers, &mut DRIVER_PROBE_LIST.unintr_lock());
    for driver in drivers {
        probe_driver_impl(driver);
    }
}

/// Start the bus probing thread.
pub fn start_thread() {
    Thread::new(
        || loop {
            probe_loop();
        },
        None,
        Some("driver probe".into()),
    )
    .expect("Failed to start driver probe thread");
}
