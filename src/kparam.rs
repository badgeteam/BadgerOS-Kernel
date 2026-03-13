use core::ptr::slice_from_raw_parts;

use alloc::{string::String, vec::Vec};

use crate::{
    LogLevel,
    bindings::raw::{limine_executable_cmdline_request, strlen},
};

unsafe extern "C" {
    #[link_name = "bootp_cmdline_req"]
    static CMDLINE: limine_executable_cmdline_request;
}

/// Map of kernel parameters.
static mut KPARAMS: Vec<(String, String)> = Vec::new();

/// Add a kernel parameter; should only be run on startup just after the heap is initialized.
pub unsafe fn add_kparam(key: String, raw_value: String) {
    let kparams = unsafe { &mut *&raw mut KPARAMS };
    match kparams.binary_search_by(|ent| ent.0.cmp(&key)) {
        Ok(_) => {
            logkf!(LogLevel::Warning, "Duplicate parameter {} ignored", key);
        }
        Err(index) => {
            kparams.insert(index, (key, raw_value));
        }
    };
}

/// Look up a kernel parameter.
pub fn get_kparam(key: &str) -> Option<&'static str> {
    let kparams = unsafe { &*&raw const KPARAMS };
    match kparams.binary_search_by(|ent| ent.0.as_str().cmp(key)) {
        Ok(index) => Some(kparams[index].1.as_str()),
        Err(_) => None,
    }
}

#[unsafe(no_mangle)]
/// Get all kernel parameters from the Limine request.
pub unsafe extern "C" fn bootp_limine_load_kparams() {
    // Try to get the kernel command-line as UTF-8.
    let cmdline = unsafe {
        if CMDLINE.response.is_null() {
            return;
        } else {
            &*slice_from_raw_parts(
                (*CMDLINE.response).cmdline as *mut u8,
                strlen((*CMDLINE.response).cmdline),
            )
        }
    };
    let cmdline = String::from_utf8_lossy(cmdline);

    // Split the command line along whitespace.
    for substr in cmdline.split_ascii_whitespace() {
        if let Some((key, val)) = substr.split_once('=') {
            unsafe { add_kparam(key.into(), val.into()) };
        } else {
            unsafe { add_kparam(substr.into(), "".into()) };
        }
    }
}
