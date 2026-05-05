// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::{c_int, c_void};

use crate::{bindings::error::Errno, config::PAGE_SIZE, mem::vmm, process};

use super::vmm::map::Mapping;

/// Map a new range of memory at an arbitrary virtual address.
/// This may round up to a multiple of the page size.
/// Alignment may be less than `align` if the kernel doesn't support it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_mem_map(
    address: *mut c_void,
    size: usize,
    prot: u32,
    flags: u32,
    fd: c_int,
    offset: i64,
) -> *mut c_void {
    let proc = process::current().unwrap();
    Errno::extract_ptr(
        try {
            let mapping;
            if flags & vmm::map::ANONYMOUS == 0 {
                if offset < 0 || offset % PAGE_SIZE as i64 != 0 {
                    Err(Errno::EINVAL)?;
                }

                let file = proc.files.lock_shared()?.get_file(fd)?;
                let object = file.get_memobject().ok_or(Errno::EACCES)?;
                mapping = Some(Mapping {
                    offset: offset as u64,
                    object,
                });
            } else {
                mapping = None;
            }

            proc.memmap()
                .map(size, address as usize, flags, prot as u8, mapping)? as *mut c_void
        },
    )
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
