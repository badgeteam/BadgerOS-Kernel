// WARNING: This is a generated file, do not edit it!
// SPDX-License-Identifier: CC0

use crate::{
    error::EResult,
    process::{uapi::time::timespec, usercopy::UserPtrMut},
    util::time::Timespec,
};
use core::ffi::*;

pub(super) fn gettime(_clkid: c_int, mut timespec: UserPtrMut<timespec>) -> EResult<()> {
    timespec.write(Timespec::now().into())
}
