// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::{c_int, c_void};

use crate::{
    bindings::{error::Errno, log::LogLevel},
    config::PAGE_SIZE,
    process,
};

use super::vmm;

pub const PROT_NONE: c_int = 0;
pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;
pub const PROT_EXEC: c_int = 4;

pub const MAP_SHARED: c_int = 1;
pub const MAP_PRIVATE: c_int = 2;
pub const MAP_ANON: c_int = 4;
pub const MAP_FIXED: c_int = 8;
pub const MAP_POPULATE: c_int = 16;

/// Map a new range of memory at an arbitrary virtual address.
/// This may round up to a multiple of the page size.
/// Alignment may be less than `align` if the kernel doesn't support it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_mem_map(
    address: *mut c_void,
    size: usize,
    prot: c_int,
    flags: c_int,
    _fd: c_int,
    _offset: i64,
) -> *mut c_void {
    let address = address as usize;
    if flags & MAP_ANON == 0 {
        logkf!(LogLevel::Warning, "TODO: non-anonymous mmap");
        return -(Errno::ENOSYS as isize) as *mut c_void;
    }
    let fixed = flags & MAP_FIXED != 0;

    let mut vmm_flags = 0;
    if prot & PROT_READ != 0 {
        vmm_flags |= vmm::flags::R;
    } else if prot & (PROT_WRITE | PROT_EXEC) != 0 {
        return -(Errno::EINVAL as isize) as *mut c_void;
    }
    if prot & PROT_WRITE != 0 {
        vmm_flags |= vmm::flags::W;
    }
    if prot & PROT_EXEC != 0 {
        vmm_flags |= vmm::flags::X;
    }
    if flags & MAP_SHARED != 0 && flags & MAP_PRIVATE == 0 {
        vmm_flags |= vmm::flags::SHM;
    } else if flags & MAP_SHARED == 0 && flags & MAP_PRIVATE != 0 {
        // VMM has no explicit flag for private mappings.
    } else {
        return -(Errno::EINVAL as isize) as *mut c_void;
    }
    if flags & MAP_POPULATE == 0 {
        vmm_flags |= vmm::flags::LAZY;
    }

    let proc = process::current().unwrap();
    let vpn = unsafe {
        if prot & PROT_READ != 0 {
            proc.memmap().map_ram(
                fixed.then_some(address / PAGE_SIZE as usize),
                size.div_ceil(PAGE_SIZE as usize),
                vmm_flags,
            )
        } else {
            proc.memmap().reserve(
                fixed.then_some(address / PAGE_SIZE as usize),
                size.div_ceil(PAGE_SIZE as usize),
            )
        }
    };

    (match vpn {
        Ok(vpn) => vpn * PAGE_SIZE as usize,
        Err(errno) => (errno as usize).wrapping_neg(),
    }) as *mut c_void
}

/// Unmap a range of memory previously allocated with `SYSCALL_MEM_MAP`.
/// Returns whether a range of memory was unmapped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_mem_unmap(address: *mut c_void, size: usize) {
    let address = address as usize;
    let proc = process::current().unwrap();
    let range = address / PAGE_SIZE as usize
        ..address
            .saturating_add(size)
            .div_ceil(PAGE_SIZE as usize)
            .min(vmm::pagetable::canon_half_pages());
    unsafe { proc.memmap().unmap(range) };
}
