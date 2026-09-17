// This file is almost exclusively interfacing with uacpi_sys.
#![allow(unsafe_op_in_unsafe_fn)]

use core::{
    ffi::{CStr, c_char},
    mem::MaybeUninit,
};

use table::MadtEntries;

use crate::{
    arch::{Arch, kcore::timer::ArchTimer},
    kcore::smp,
    uacpi_sys::{self, *},
};

pub mod table;
mod uacpi_kernel_api;

fn uacpi_status_to_string(status: uacpi_status) -> &'static str {
    unsafe {
        let cstr = CStr::from_ptr(uacpi_sys::uacpi_status_to_string(status));
        cstr.to_str().unwrap()
    }
}

fn find_table(signature: &[u8]) -> Option<uacpi_table> {
    unsafe {
        assert!(signature.ends_with(&[0]));
        let mut handle = MaybeUninit::<uacpi_table>::uninit();
        let res =
            uacpi_table_find_by_signature(signature.as_ptr() as *const c_char, handle.as_mut_ptr());
        if res == UACPI_STATUS_OK {
            Some(handle.assume_init())
        } else {
            None
        }
    }
}

pub(super) unsafe fn init() {
    Arch::timer_init_pre_acpi();

    let status = uacpi_initialize(0);
    if status != UACPI_STATUS_OK {
        panic!("UACPI init failed: {}", uacpi_status_to_string(status));
    }

    Arch::timer_init_acpi();

    unsafe {
        let mut madt = find_table(ACPI_MADT_SIGNATURE).expect("Cannot find MADT");
        smp::init_acpi(MadtEntries::new(
            &*(madt.__bindgen_anon_1.virt_addr as *const acpi_madt),
        ));
        uacpi_table_unref(&mut madt);
    }
}
