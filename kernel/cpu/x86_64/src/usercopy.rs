// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::process::usercopy::{AccessFault, AccessResult};

/// A single-byte fallible store.
pub unsafe fn fallible_store_u8(addr: *mut u8, value: u8) -> AccessResult<()> {
    todo!()
}

/// A single-byte fallible load.
pub unsafe fn fallible_load_u8(addr: *const u8) -> AccessResult<u8> {
    todo!()
}

/// Raw `memcpy` from kernel to user memory.
/// Catches faults on `dest` but will panic for faults on `src`.
pub unsafe fn copy_to_user(dest: *mut (), src: *const (), len: usize) -> AccessResult<()> {
    let dest = dest as *mut u8;
    let src = src as *const u8;

    unsafe {
        if (dest as *const u8) < src {
            for i in 0..len {
                let tmp = *src.wrapping_add(i);
                fallible_store_u8(dest.wrapping_add(i), tmp)?;
            }
        } else {
            for i in (0..len).rev() {
                let tmp = *src.wrapping_add(i);
                fallible_store_u8(dest.wrapping_add(i), tmp)?;
            }
        }
    }

    Ok(())
}

/// Raw `memcpy` from user to kernel memory.
/// Catches faults on `src` but will panic for faults on `dest`.
pub unsafe fn copy_from_user(dest: *mut (), src: *const (), len: usize) -> AccessResult<()> {
    let dest = dest as *mut u8;
    let src = src as *const u8;

    unsafe {
        if (dest as *const u8) < src {
            for i in 0..len {
                let tmp = fallible_load_u8(src.wrapping_add(i))?;
                *dest.wrapping_add(i) = tmp;
            }
        } else {
            for i in (0..len).rev() {
                let tmp = fallible_load_u8(src.wrapping_add(i))?;
                *dest.wrapping_add(i) = tmp;
            }
        }
    }

    Ok(())
}

mod c_api {
    use crate::bindings::{error::Errno, raw::errno_t};

    /// A single-byte fallible store.
    pub unsafe fn fallible_store_u8(addr: *mut u8, value: u8) -> errno_t {
        Errno::extract(unsafe { super::fallible_store_u8(addr, value) })
    }

    /// A single-byte fallible load.
    pub unsafe fn fallible_load_u8(addr: *const u8) -> errno_t {
        Errno::extract_u32(unsafe { super::fallible_load_u8(addr).map(Into::into) })
    }

    /// Raw `memcpy` from kernel to user memory.
    /// Catches faults on `dest` but will panic for faults on `src`.
    pub unsafe fn copy_to_user(dest: *mut (), src: *const (), len: usize) -> errno_t {
        Errno::extract(unsafe { super::copy_to_user(dest, src, len) })
    }

    /// Raw `memcpy` from user to kernel memory.
    /// Catches faults on `src` but will panic for faults on `dest`.
    pub unsafe fn copy_from_user(dest: *mut (), src: *const (), len: usize) -> errno_t {
        Errno::extract(unsafe { super::copy_from_user(dest, src, len) })
    }
}
