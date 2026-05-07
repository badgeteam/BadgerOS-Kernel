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

struct DevZero {}

impl DevZero {
    pub fn new(_device: Device) -> EResult<Box<Self>> {
        Ok(Box::new(Self {}))
    }
}

impl BaseDriver for DevZero {}

impl CharDriver for DevZero {
    fn read(&self, mut buf: UserSliceMut<'_, u8>, _nonblock: bool) -> EResult<usize> {
        buf.fill(0)?;
        Ok(buf.len())
    }

    fn write(&self, buf: UserSlice<'_, u8>, _nonblock: bool) -> EResult<usize> {
        Ok(buf.len())
    }

    fn poll(&self) -> u32 {
        // /dev/zero - always writable and readable.
        poll::IN | poll::OUT
    }

    fn poll_waitlists<'a>(
        &'a self,
        _interest: u32,
        _collect: &mut Vec<&'a Waitlist>,
    ) -> EResult<()> {
        // /dev/zero - always writable and readable.
        Ok(())
    }
}

fn match_dummy(_info: DeviceInfoView<'_>) -> bool {
    false
}

pub(super) static DEV_ZERO_DRIVER: driver_char_t =
    char_driver_struct!(DevZero, match_dummy, DevZero::new);

pub(super) fn add_driver() {
    device::add_driver(&DEV_ZERO_DRIVER.base).unwrap();
}
