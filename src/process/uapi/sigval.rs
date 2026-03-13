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
impl core::fmt::Debug for sigval {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("sigval")
            .field("sival_int", unsafe { &self.sival_int })
            .field("sival_ptr", unsafe { &self.sival_ptr })
            .finish()
    }
}
unsafe impl UserCopyable for sigval {}
