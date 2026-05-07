use core::ffi::c_void;

use alloc::vec::Vec;

use crate::{
    bindings::{
        device::{AbstractDevice, BaseDriver, Device, DeviceFilters},
        error::{EResult, Errno},
        raw::{self, dev_class_t_DEV_CLASS_CHAR, device_char_t, driver_char_t},
    },
    kernel::sync::waitlist::Waitlist,
    process::usercopy::{UserSlice, UserSliceMut},
};

/// Specialization for character devices.
pub type CharDevice = AbstractDevice<device_char_t>;
impl CharDevice {
    /// Get a list of devices using a filter.
    pub fn filter(filters: DeviceFilters) -> EResult<Vec<CharDevice>> {
        unsafe {
            Device::filter_impl::<device_char_t, CharDevice, true>(
                filters,
                dev_class_t_DEV_CLASS_CHAR,
            )
        }
    }

    /// Read bytes from the device.
    pub fn read(&self, rdata: UserSliceMut<'_, u8>, nonblock: bool) -> EResult<usize> {
        Errno::check_usize(unsafe {
            raw::device_char_read(
                self.as_raw_ptr(),
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
                self.as_raw_ptr(),
                wdata.as_ptr() as *const c_void,
                wdata.len(),
                nonblock,
            )
        })
    }

    /// Get current polling status flags.
    /// Returns `0` if no driver is bound.
    pub fn poll(&self) -> u32 {
        unsafe { poll_via_char(self.as_raw_ptr()) }
    }

    /// Collect waitlists for the requested poll interest flags.
    /// Returns `Ok(())` with no waitlists collected if no driver is bound.
    pub fn poll_waitlists<'a>(
        &'a self,
        interest: u32,
        collect: &mut Vec<&'a Waitlist>,
    ) -> EResult<()> {
        unsafe { poll_waitlists_via_char(self.as_raw_ptr(), interest, collect) }
    }
}

/// Dispatch `poll` for any device whose driver inherits from `driver_char_t`.
/// Used by both [`CharDevice`] and [`super::tty::TTYDevice`].
pub(super) unsafe fn poll_via_char(device: *mut device_char_t) -> u32 {
    unsafe {
        raw::mutex_lock_shared(&raw mut (*device).base.driver_mtx);
        let driver = (*device).base.driver as *const driver_char_t;
        let res = if driver.is_null() {
            0
        } else {
            ((*driver).poll.unwrap())(device)
        };
        raw::mutex_unlock_shared(&raw mut (*device).base.driver_mtx);
        res
    }
}

/// Dispatch `poll_waitlists` for any device whose driver inherits from `driver_char_t`.
/// Used by both [`CharDevice`] and [`super::tty::TTYDevice`].
pub(super) unsafe fn poll_waitlists_via_char<'a>(
    device: *mut device_char_t,
    interest: u32,
    collect: &mut Vec<&'a Waitlist>,
) -> EResult<()> {
    unsafe {
        raw::mutex_lock_shared(&raw mut (*device).base.driver_mtx);
        let driver = (*device).base.driver as *const driver_char_t;
        let res = if driver.is_null() {
            Ok(())
        } else {
            Errno::check(((*driver).poll_waitlists.unwrap())(
                device,
                interest,
                collect as *mut Vec<&'a Waitlist> as *mut c_void,
            ))
        };
        raw::mutex_unlock_shared(&raw mut (*device).base.driver_mtx);
        res
    }
}

/// Character device driver functions.
pub trait CharDriver: BaseDriver {
    /// Read bytes from the device.
    fn read(&self, rdata: UserSliceMut<'_, u8>, nonblock: bool) -> EResult<usize>;
    /// Write bytes to the device.
    fn write(&self, wdata: UserSlice<'_, u8>, nonblock: bool) -> EResult<usize>;
    /// Get current polling status flags.
    fn poll(&self) -> u32;
    /// Collect waitlists for the requested poll interest flags.
    fn poll_waitlists<'a>(
        &'a self,
        interest: u32,
        collect: &mut Vec<&'a Waitlist>,
    ) -> EResult<()>;
}

/// Helper macro for filling in character driver fields.
#[macro_export]
macro_rules! char_driver_struct {
    ($type: ty, $match_: expr, $add: expr) => {
        crate::abstract_char_driver_struct! {
            $type, dev_class_t_DEV_CLASS_CHAR, $match_, $add
        }
    };
}

/// Helper macro for filling in character driver fields.
#[macro_export]
macro_rules! abstract_char_driver_struct {
    ($type: ty, $class: expr, $match_: expr, $add: expr) => {{
        use crate::{
            bindings::{device::class::char::*, error::*, raw::*},
            kernel::sync::waitlist::Waitlist,
            process::usercopy::{UserSlice, UserSliceMut},
        };
        use ::alloc::vec::Vec;
        use ::core::{
            ffi::c_void,
            ptr::{slice_from_raw_parts, slice_from_raw_parts_mut},
        };
        driver_char_t {
            base: crate::abstract_driver_struct! {
                $type,
                $class,
                $match_,
                $add
            },
            write: {
                unsafe extern "C" fn write_wrapper(
                    device: *mut device_char_t,
                    wdata: *const c_void,
                    wdata_len: usize,
                    nonblock: bool,
                ) -> errno_size_t {
                    let ptr = unsafe { &mut *((*device).base.cookie as *mut $type) };
                    Errno::extract_usize(ptr.write(
                        UserSlice::new_kernel(unsafe {
                            &*slice_from_raw_parts(wdata as *const u8, wdata_len)
                        }),
                        nonblock,
                    ))
                }
                Some(write_wrapper)
            },
            read: {
                unsafe extern "C" fn read_wrapper(
                    device: *mut device_char_t,
                    rdata: *mut c_void,
                    rdata_len: usize,
                    nonblock: bool,
                ) -> errno_size_t {
                    let ptr = unsafe { &mut *((*device).base.cookie as *mut $type) };
                    Errno::extract_usize(ptr.read(
                        UserSliceMut::new_kernel_mut(unsafe {
                            &mut *slice_from_raw_parts_mut(rdata as *mut u8, rdata_len)
                        }),
                        nonblock,
                    ))
                }
                Some(read_wrapper)
            },
            poll: {
                unsafe extern "C" fn poll_wrapper(device: *mut device_char_t) -> u32 {
                    let ptr = unsafe { &*((*device).base.cookie as *const $type) };
                    ptr.poll()
                }
                Some(poll_wrapper)
            },
            poll_waitlists: {
                unsafe extern "C" fn poll_waitlists_wrapper(
                    device: *mut device_char_t,
                    interest: u32,
                    collect: *mut c_void,
                ) -> errno_t {
                    let ptr = unsafe { &*((*device).base.cookie as *const $type) };
                    let collect = unsafe { &mut *(collect as *mut Vec<&Waitlist>) };
                    Errno::extract(ptr.poll_waitlists(interest, collect))
                }
                Some(poll_waitlists_wrapper)
            },
        }
    }};
}
