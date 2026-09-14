// WARNING: This is a generated file, do not edit it!
// SPDX-License-Identifier: CC0

use crate::{
    arch::{Arch, ArchTrait},
    error::EResult,
    process::{
        uapi::uname::utsname,
        usercopy::{UserPtrMut, UserSlice},
    },
    util::version,
};

pub(super) fn uname(mut name: UserPtrMut<utsname>) -> EResult<()> {
    let mut utsname = utsname::default();
    utsname.sysname.assign("BadgerOS");
    utsname.machine.assign(Arch::MACHINE);
    utsname.release.assign(version::RELEASE);
    utsname.release.assign(version::VERSION);

    name.write(utsname)
}

pub(super) fn log(message: UserSlice<u8>) -> EResult<()> {
    // TODO: Replace with proper earlycon.
    let mut prev = 0u8;
    for i in 0..message.len() {
        let c = message.read(i)?;
        unsafe {
            if c == b'\n' && prev != b'\r' {
                crate::boot::protocol::bootp_early_putc(b'\r');
            } else if prev == b'\r' && c != b'\n' {
                crate::boot::protocol::bootp_early_putc(b'\n');
            }
            crate::boot::protocol::bootp_early_putc(c);
            prev = c;
        }
    }
    Ok(())
}
