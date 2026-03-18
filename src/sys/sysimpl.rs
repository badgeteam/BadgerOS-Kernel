// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::c_int;

use crate::{
    bindings::error::Errno,
    cpu,
    process::{uapi::uname::utsname, usercopy::UserPtr},
};

use super::version;

#[unsafe(no_mangle)]
#[inline(never)]
pub unsafe extern "C" fn syscall_sys_uname(out: *mut utsname) -> c_int {
    Errno::extract(
        try {
            let mut utsname = utsname::default();
            utsname.sysname.assign("BadgerOS");
            utsname.machine.assign(cpu::MACHINE_NAME);
            utsname.release.assign(version::RELEASE);
            utsname.release.assign(version::VERSION);

            UserPtr::new_mut(out)?.write(utsname)?;
        },
    )
}
