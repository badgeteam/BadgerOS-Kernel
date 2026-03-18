use alloc::vec::Vec;
use dev_null::DEV_NULL_DRIVER;
use dev_zero::DEV_ZERO_DRIVER;

use crate::bindings::{
    self,
    device::{Device, DeviceInfo, HasBaseDevice, class::char::CharDevice},
};

pub mod ahci;
pub mod dev_null;
pub mod dev_zero;
pub mod serial;

#[unsafe(no_mangle)]
unsafe extern "C" fn add_rust_builtin_drivers() {
    ahci::add_drivers();
    dev_null::add_driver();
    dev_zero::add_driver();
    serial::add_drivers();
}

static mut NULL_INSTANCE: Option<CharDevice> = None;
static mut ZERO_INSTANCE: Option<CharDevice> = None;

pub fn null_instance() -> CharDevice {
    unsafe { (&*&raw const NULL_INSTANCE).clone().unwrap() }
}

pub fn zero_instance() -> CharDevice {
    unsafe { (&*&raw const ZERO_INSTANCE).clone().unwrap() }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn device_create_null_zero() {
    let dev_null = Device::add(DeviceInfo {
        parent: None,
        irq_parent: None,
        addrs: Vec::new(),
        phandle: None,
    })
    .unwrap();
    dev_null.activate();
    unsafe {
        bindings::raw::device_set_driver(dev_null.as_raw_ptr(), &raw const DEV_NULL_DRIVER.base);
        NULL_INSTANCE = Some(dev_null.as_char().unwrap());
    }

    let dev_zero = Device::add(DeviceInfo {
        parent: None,
        irq_parent: None,
        addrs: Vec::new(),
        phandle: None,
    })
    .unwrap();
    dev_zero.activate();
    unsafe {
        bindings::raw::device_set_driver(dev_zero.as_raw_ptr(), &raw const DEV_ZERO_DRIVER.base);
        ZERO_INSTANCE = Some(dev_zero.as_char().unwrap());
    }
}
