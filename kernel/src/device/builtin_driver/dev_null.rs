use alloc::boxed::Box;

use crate::{
    bindings::{
        device::{self, BaseDriver, Device, DeviceInfoView, class::char::CharDriver},
        error::EResult,
        raw::driver_char_t,
    },
    char_driver_struct,
    process::usercopy::{UserSlice, UserSliceMut},
};

struct DevNull {}

impl DevNull {
    pub fn new(_device: Device) -> EResult<Box<Self>> {
        Ok(Box::new(Self {}))
    }
}

impl BaseDriver for DevNull {}

impl CharDriver for DevNull {
    fn read(&self, _buf: UserSliceMut<'_, u8>) -> EResult<usize> {
        Ok(0)
    }

    fn write(&self, _buf: UserSlice<'_, u8>) -> EResult<usize> {
        Ok(_buf.len())
    }
}

fn match_dummy(_info: DeviceInfoView<'_>) -> bool {
    false
}

pub(super) static DEV_NULL_DRIVER: driver_char_t =
    char_driver_struct!(DevNull, match_dummy, DevNull::new);

pub(super) fn add_driver() {
    device::add_driver(&DEV_NULL_DRIVER.base).unwrap();
}
