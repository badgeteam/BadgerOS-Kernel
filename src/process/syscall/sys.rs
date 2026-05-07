// WARNING: This is a generated file, do not edit it!
// SPDX-License-Identifier: CC0

use crate::{
    bindings::{error::EResult, raw::rawputc},
    cpu,
    process::{
        uapi::uname::utsname,
        usercopy::{UserPtrMut, UserSlice},
    },
    util::version,
};

pub(super) fn uname(mut name: UserPtrMut<utsname>) -> EResult<()> {
    let mut utsname = utsname::default();
    utsname.sysname.assign("BadgerOS");
    utsname.machine.assign(cpu::MACHINE_NAME);
    utsname.release.assign(version::RELEASE);
    utsname.release.assign(version::VERSION);

    name.write(utsname)
}

pub(super) fn log(message: UserSlice<u8>) -> EResult<()> {
    for i in 0..message.len() {
        unsafe {
            rawputc(message.read(i)?);
        }
    }
    Ok(())
}
