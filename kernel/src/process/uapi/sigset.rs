// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#![allow(non_camel_case_types)]

use core::ffi::c_ulong;

use crate::process::usercopy::UserCopyable;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct sigset_t {
    pub __sig: [c_ulong; 1024 / (8 * size_of::<c_ulong>())],
}
unsafe impl UserCopyable for sigset_t {}
