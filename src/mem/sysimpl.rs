// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::{c_int, c_void};

use crate::{
    bindings::{error::Errno, log::LogLevel},
    mem::vmm,
    process,
};

/// Map a new range of memory at an arbitrary virtual address.
/// This may round up to a multiple of the page size.
/// Alignment may be less than `align` if the kernel doesn't support it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_mem_map(
    address: *mut c_void,
    size: usize,
    prot: u32,
    flags: u32,
    _fd: c_int,
    _offset: i64,
) -> *mut c_void {
    if flags & vmm::map::ANONYMOUS == 0 {
        logkf!(LogLevel::Warning, "TODO: Non-anonymous mmap()");
        return -(Errno::ENOSYS as i32) as isize as *mut c_void;
    }

    let proc = process::current().unwrap();
    match proc
        .memmap()
        .map(size, address as usize, flags, prot as u8, None)
    {
        Ok(addr) => addr as *mut c_void,
        Err(err) => -(err as i32) as isize as *mut c_void,
    }
}

/// Unmap a range of memory previously allocated with `SYSCALL_MEM_MAP`.
/// Returns whether a range of memory was unmapped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_mem_unmap(address: *mut c_void, size: usize) -> c_int {
    let proc = process::current().unwrap();
    match proc
        .memmap()
        .unmap(address as usize..address as usize + size)
    {
        Ok(()) => 0,
        Err(err) => -(err as i32),
    }
}

/// Change the protection flags on a range of memory.
/// May fail if the process does not have write access to mapped objects.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_mem_protect(
    address: *mut c_void,
    size: usize,
    prot: u32,
) -> c_int {
    let proc = process::current().unwrap();
    match proc
        .memmap()
        .protect(address as usize..address as usize + size, prot as u8)
    {
        Ok(()) => 0,
        Err(err) => -(err as i32),
    }
}
