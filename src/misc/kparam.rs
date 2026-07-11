// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::{string::String, vec::Vec};

use crate::{LogLevel, bindings::raw::limine_executable_cmdline_request};

unsafe extern "C" {
    #[link_name = "bootp_cmdline_req"]
    static CMDLINE: limine_executable_cmdline_request;
}

/// Map of kernel parameters.
static mut KPARAMS: Vec<(String, String)> = Vec::new();

/// Parse kernel parameters from a string.
/// # Safety
/// Should only be run on startup just after the heap is initialized.
pub unsafe fn parse_kparams(raw: &[u8]) {
    let mut nonascii = false;
    for slice in raw.split(|x| *x == 0 || x.is_ascii_whitespace()) {
        if slice.is_empty() {
            continue;
        }
        let Some(slice) = slice.as_ascii() else {
            nonascii = true;
            continue;
        };

        if let Some(delim) = slice.iter().position(|x| x.to_u8() == b'=') {
            let key = slice[..delim].as_str();
            let val = slice[delim + 1..].as_str();
            unsafe { add_kparam(key.into(), val.into()) };
        } else {
            unsafe { add_kparam(slice.as_str().into(), String::new()) };
        }
    }

    if nonascii {
        logkf!(
            LogLevel::Warning,
            "Some kernel parameters ignored because they contained non-ascii data"
        );
    }
}

/// Add a kernel parameter.
/// # Safety
/// Should only be run on startup just after the heap is initialized.
pub unsafe fn add_kparam(key: String, raw_value: String) {
    let kparams = unsafe { &mut *&raw mut KPARAMS };
    match kparams.binary_search_by(|ent| ent.0.cmp(&key)) {
        Ok(_) => {
            logkf!(LogLevel::Warning, "Duplicate parameter {} ignored", key);
        }
        Err(index) => {
            if raw_value.is_empty() {
                logkf!(LogLevel::Info, "Kernel parameter: {}={}", &key, &raw_value);
            } else {
                logkf!(LogLevel::Info, "Kernel parameter: {}", &key);
            }

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
