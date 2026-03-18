use core::ffi::c_void;

use alloc::vec::Vec;

use crate::{
    bindings::{
        device::{AbstractDevice, Device, DeviceFilters},
        error::{EResult, Errno},
        raw::{self, dev_class_t_DEV_CLASS_TTY, device_char_t, device_tty_t},
    },
    process::{
        uapi::termios::termios,
        usercopy::{UserSlice, UserSliceMut},
    },
};

use super::char::CharDriver;

/// Specialization for TTY devices.
pub type TTYDevice = AbstractDevice<device_tty_t>;
impl TTYDevice {
    /// Get a list of devices using a filter.
    pub fn filter(filters: DeviceFilters) -> EResult<Vec<TTYDevice>> {
        unsafe {
            Device::filter_impl::<device_tty_t, TTYDevice, true>(filters, dev_class_t_DEV_CLASS_TTY)
        }
    }

    /// Read bytes from the device.
    pub fn read(&self, rdata: UserSliceMut<'_, u8>, nonblock: bool) -> EResult<usize> {
        Errno::check_usize(unsafe {
            raw::device_char_read(
                self.as_raw_ptr() as *mut device_char_t,
                rdata.as_mut_ptr() as *mut c_void,
                rdata.len(),
                nonblock,
            )
        })
    }

    /// Write bytes to the device.
    pub fn write(&self, wdata: UserSlice<'_, u8>, nonblock: bool) -> EResult<usize> {
        Errno::check_usize(unsafe {
            raw::device_char_write(
                self.as_raw_ptr() as *mut device_char_t,
                wdata.as_ptr() as *const c_void,
                wdata.len(),
                nonblock,
            )
        })
    }

    /// Get terminal attributes.
    pub fn getattr(&self, attr: &mut termios) -> EResult<()> {
        Errno::check(unsafe {
            raw::device_tty_getattr(self.as_raw_ptr(), attr as *mut termios as *mut raw::termios)
        })
    }

    /// Set terminal attributes.
    pub fn setattr(&self, attr: &termios) -> EResult<()> {
        Errno::check(unsafe {
            raw::device_tty_setattr(
                self.as_raw_ptr(),
                attr as *const termios as *const raw::termios,
            )
        })
    }
}

pub trait TTYDriver: CharDriver {
    /// Try to set the given terminal attributes.
    /// If successful, `tio` will be be updated by the device subsytem.
    fn setattr(&self, newattr: &termios) -> EResult<()>;
}

/// Helper macro for filling in TTY driver fields.
#[macro_export]
macro_rules! tty_driver_struct {
    ($type: ty, $match_: expr, $add: expr) => {{
        use crate::{
            bindings::{device::class::tty::*, error::*, raw::*},
            process::uapi,
        };
        driver_tty_t {
            base: crate::abstract_char_driver_struct! {
                $type,
                dev_class_t_DEV_CLASS_TTY,
                $match_,
                $add
            },
            setattr: {
                unsafe extern "C" fn setattr_wrapper(
                    device: *mut device_tty_t,
                    newattr: *const termios,
                ) -> errno_t {
                    let ptr = unsafe { &mut *((*device).base.base.cookie as *mut $type) };
                    Errno::extract(
                        ptr.setattr(unsafe { &*(newattr as *const uapi::termios::termios) }),
                    )
                }
                Some(setattr_wrapper)
            },
        }
    }};
}
