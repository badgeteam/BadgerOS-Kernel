// This file is almost exclusively interfacing with uacpi_sys.
#![allow(unsafe_op_in_unsafe_fn)]

use core::ffi::CStr;

use crate::{
    arch::{Arch, kcore::timer::ArchTimer},
    uacpi_sys::{self, *},
};

mod uacpi_kernel_api;

fn uacpi_status_to_string(status: uacpi_status) -> &'static str {
    unsafe {
        let cstr = CStr::from_ptr(uacpi_sys::uacpi_status_to_string(status));
        cstr.to_str().unwrap()
    }
}

pub(super) unsafe fn init() {
    let status = uacpi_initialize(0);
    if status != UACPI_STATUS_OK {
        panic!("UACPI init failed: {}", uacpi_status_to_string(status));
    }

    Arch::timer_init_acpi();
}
