use alloc::boxed::Box;

use alloc::vec::Vec;

use crate::{
    bindings::{
        device::{self, BaseDriver, Device, DeviceInfoView, class::char::CharDriver},
        error::EResult,
        raw::driver_char_t,
    },
    char_driver_struct,
    filesystem::poll,
    kernel::sync::waitlist::Waitlist,
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
    fn read(&self, _buf: UserSliceMut<'_, u8>, _nonblock: bool) -> EResult<usize> {
        Ok(0)
    }

    fn write(&self, _buf: UserSlice<'_, u8>, _nonblock: bool) -> EResult<usize> {
        Ok(_buf.len())
    }

    fn poll(&self) -> u32 {
        // /dev/null - always writable, never readable, but reading also doesn't block.
        poll::IN | poll::OUT
    }

    fn poll_waitlists<'a>(
        &'a self,
        _interest: u32,
        _collect: &mut Vec<&'a Waitlist>,
    ) -> EResult<()> {
        // /dev/null - always writable, never readable, but reading also doesn't block.
        Ok(())
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
