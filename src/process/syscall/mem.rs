// WARNING: This is a generated file, do not edit it!
// SPDX-License-Identifier: CC0

use crate::{
    bindings::error::{EResult, Errno},
    config::PAGE_SIZE,
    mem::vmm::{self, map::Mapping},
    process,
};
use core::ffi::*;

pub(super) fn map(
    address: usize,
    size: usize,
    prot: c_int,
    flags: c_int,
    fd: c_int,
    offset: i64,
) -> EResult<usize> {
    let proc = process::current().unwrap();

    let mapping;
    if flags as u32 & vmm::map::ANONYMOUS == 0 {
        if offset < 0 || offset % PAGE_SIZE as i64 != 0 {
            Err(Errno::EINVAL)?;
        }

        let file = proc.files.lock_shared()?.get_file(fd)?;
        let object = file.get_memobject().ok_or(Errno::ENODEV)?;
        mapping = Some(Mapping {
            offset: offset as u64,
            object,
        });
    } else {
        mapping = None;
    }

    proc.memmap()
        .map(size, address as usize, flags as u32, prot as u8, mapping)
}

pub(super) fn unmap(address: usize, size: usize) -> EResult<()> {
    let proc = process::current().unwrap();
    proc.memmap()
        .unmap(address as usize..address as usize + size)
}

pub(super) fn protect(address: usize, size: usize, prot: c_int) -> EResult<()> {
    let proc = process::current().unwrap();
    proc.memmap()
        .protect(address as usize..address as usize + size, prot as u8)
}
