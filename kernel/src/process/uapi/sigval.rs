// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::{c_int, c_void};

use crate::process::usercopy::UserCopyable;

#[repr(C)]
#[derive(Clone, Copy)]
pub union sigval {
    pub sival_int: c_int,
    pub sival_ptr: *mut c_void,
}
unsafe impl UserCopyable for sigval {}
