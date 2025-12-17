// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ffi::{c_char, c_void},
    marker::PhantomData,
    mem::MaybeUninit,
    ops::Range,
    ptr::NonNull,
};

use alloc::{ffi::CString, vec::Vec};

use crate::{
    bindings::{
        error::{EResult, Errno},
        raw::{isr_noexc_copy_u8, isr_noexc_mem_copy},
    },
    cpu, mem,
};

pub type AccessResult<T> = Result<T, Errno>;

#[allow(non_upper_case_globals)]
pub const AccessFault: Errno = Errno::EFAULT;

/*
TODO: Reinstate `AccessFault` when `try bikeshed _ {}` becomes more stable.
/// Represents an access fault originating from a [`UserSlice`] or [`UserPtr`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AccessFault;

impl Display for AccessFault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("AccessFault")
    }
}

impl Error for AccessFault {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }

    fn provide<'a>(&'a self, _request: &mut core::error::Request<'a>) {}
}
*/

/// Anything deemed safe for copying between user and kernel memory.
pub unsafe trait UserCopyable: Sized {}
unsafe impl UserCopyable for bool {}
unsafe impl UserCopyable for u8 {}
unsafe impl UserCopyable for i8 {}
unsafe impl UserCopyable for u16 {}
unsafe impl UserCopyable for i16 {}
unsafe impl UserCopyable for u32 {}
unsafe impl UserCopyable for i32 {}
unsafe impl UserCopyable for u64 {}
unsafe impl UserCopyable for i64 {}
unsafe impl UserCopyable for u128 {}
unsafe impl UserCopyable for i128 {}
unsafe impl UserCopyable for usize {}
unsafe impl UserCopyable for isize {}
unsafe impl<T: UserCopyable> UserCopyable for *mut T {}
unsafe impl<T: UserCopyable> UserCopyable for *const T {}
unsafe impl<T: UserCopyable, const L: usize> UserCopyable for [T; L] {}

/// Represents a slice of user memory.
/// Can be safely constructed from a (pointer, length) pair.
pub type UserSliceMut<'a, T> = UserSlice<'a, T, true>;

/// Represents a slice of user memory.
/// Can be safely constructed from a (pointer, length) pair.
#[derive(Clone, Copy)]
pub struct UserSlice<'a, T: UserCopyable, const MUTABLE: bool = false> {
    ptr: NonNull<T>,
    length: usize,
    marker: PhantomData<&'a T>,
}

impl<'a, T: UserCopyable> UserSlice<'a, T, false> {
    /// Create a slice from kernel memory; it is assumed to be safe to access.
    pub fn new_kernel(kernel_slice: &'a [T]) -> Self {
        Self {
            ptr: unsafe { NonNull::new_unchecked(kernel_slice.as_ptr() as *mut T) },
            length: kernel_slice.len(),
            marker: PhantomData,
        }
    }

    /// Create a new user-access slice; will validate that the range is user memory.
    pub fn new(ptr: *const T, length: usize) -> AccessResult<Self> {
        let vaddr = ptr as usize;
        if !mem::vmm::pagetable::is_canon_user_range(
            vaddr
                ..vaddr
                    .checked_add(length.checked_mul(size_of::<T>()).ok_or(AccessFault)?)
                    .ok_or(AccessFault)?,
        ) {
            return Err(AccessFault);
        }
        Ok(Self {
            ptr: NonNull::new(ptr as *mut T).ok_or(AccessFault)?,
            length,
            marker: PhantomData,
        })
    }
}

impl<'a, T: UserCopyable> UserSlice<'a, T, true> {
    /// Create a slice from kernel memory; it is assumed to be safe to access.
    pub fn new_kernel_mut(kernel_slice: &'a mut [T]) -> Self {
        Self {
            ptr: unsafe { NonNull::new_unchecked(kernel_slice.as_ptr() as *mut T) },
            length: kernel_slice.len(),
            marker: PhantomData,
        }
    }

    /// Create a new user-access slice; will validate that the range is user memory.
    pub fn new_mut(ptr: *mut T, length: usize) -> AccessResult<Self> {
        let vaddr = ptr as usize;
        if !mem::vmm::pagetable::is_canon_user_range(
            vaddr
                ..vaddr
                    .checked_add(length.checked_mul(size_of::<T>()).ok_or(AccessFault)?)
                    .ok_or(AccessFault)?,
        ) {
            return Err(AccessFault);
        }
        Ok(Self {
            ptr: NonNull::new(ptr as *mut T).ok_or(AccessFault)?,
            length,
            marker: PhantomData,
        })
    }
}

impl<'a, T: UserCopyable, const MUTABLE: bool> UserSlice<'a, T, MUTABLE> {
    /// Get a subslice from this one.
    pub fn subslice(&self, range: Range<usize>) -> UserSlice<'a, T, false> {
        assert!(
            range.end <= self.length,
            "UserSlice::subslice end {} out of range [0-{}]",
            range.end,
            self.length
        );
        UserSlice {
            ptr: unsafe { self.ptr.add(range.start) },
            length: range.len(),
            marker: PhantomData,
        }
    }

    /// Try to read multiple elements from the slice.
    pub fn read_multiple(&self, index: usize, out: &mut [T]) -> AccessResult<()> {
        debug_assert!(
            index + out.len() <= self.len(),
            "UserSlice::read_multiple index {} len {} out of range [0-{})",
            index,
            out.len(),
            self.len()
        );
        let faulted = unsafe {
            isr_noexc_mem_copy(
                out.as_ptr() as *mut c_void,
                self.ptr.add(index).as_ptr() as *const c_void,
                size_of::<T>() * out.len(),
            )
        };
        if faulted { Err(AccessFault) } else { Ok(()) }
    }

    /// Try to read an element from the slice.
    pub fn read(&self, index: usize) -> AccessResult<T> {
        debug_assert!(
            index < self.len(),
            "UserSlice::read index {} out of range [0-{})",
            index,
            self.len()
        );
        let mut tmp = MaybeUninit::uninit();
        unsafe { cpu::mmu::enable_sum() };
        let faulted = unsafe {
            isr_noexc_mem_copy(
                &raw mut tmp as *mut c_void,
                self.ptr.add(index).as_ptr() as *const c_void,
                size_of::<T>(),
            )
        };
        unsafe { cpu::mmu::disable_sum() };
        if faulted {
            Err(AccessFault)
        } else {
            Ok(unsafe { tmp.assume_init() })
        }
    }

    /// Pointer inside this slice.
    pub fn as_ptr(&self) -> *const T {
        self.ptr.as_ptr()
    }

    /// Length of this slice.
    pub fn len(&self) -> usize {
        self.length
    }
}

impl<'a, T: UserCopyable> UserSlice<'a, T, true> {
    /// Get a subslice from this one.
    pub fn subslice_mut(&mut self, range: Range<usize>) -> UserSlice<'a, T, true> {
        assert!(
            range.end <= self.length,
            "UserSlice::subslice end {} out of range [0-{}]",
            range.end,
            self.length
        );
        UserSlice {
            ptr: unsafe { self.ptr.add(range.start) },
            length: range.len(),
            marker: PhantomData,
        }
    }

    /// Try to write multiple elements from the slice.
    pub fn write_multiple(&mut self, index: usize, data: &[T]) -> AccessResult<()> {
        debug_assert!(
            index + data.len() <= self.len(),
            "UserSlice::write_multiple index {} len {} out of range [0-{})",
            index,
            data.len(),
            self.len()
        );
        let faulted = unsafe {
            isr_noexc_mem_copy(
                self.ptr.add(index).as_ptr() as *mut c_void,
                data.as_ptr() as *const c_void,
                size_of::<T>() * data.len(),
            )
        };
        if faulted { Err(AccessFault) } else { Ok(()) }
    }

    /// Try to write an element to the slice.
    pub fn write(&mut self, index: usize, data: T) -> AccessResult<()> {
        debug_assert!(
            index < self.len(),
            "UserSlice::write index {} out of range [0-{})",
            index,
            self.len()
        );
        unsafe { cpu::mmu::enable_sum() };
        let faulted = unsafe {
            isr_noexc_mem_copy(
                self.ptr.add(index).as_ptr() as *mut c_void,
                &raw const data as *const c_void,
                size_of::<T>(),
            )
        };
        unsafe { cpu::mmu::disable_sum() };
        if faulted { Err(AccessFault) } else { Ok(()) }
    }

    /// Fill this slice with a certain value.
    pub fn fill(&mut self, data: T) -> AccessResult<()> {
        unsafe { cpu::mmu::enable_sum() };
        let mut faulted = false;
        unsafe {
            for i in 0..self.length {
                if isr_noexc_mem_copy(
                    self.ptr.add(i).as_ptr() as *mut c_void,
                    &raw const data as *const c_void,
                    size_of::<T>(),
                ) {
                    faulted = true;
                    break;
                }
            }
        };
        unsafe { cpu::mmu::disable_sum() };
        if faulted { Err(AccessFault) } else { Ok(()) }
    }

    /// Pointer inside this slice.
    pub fn as_mut_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }
}

impl<'a, T: UserCopyable> Into<UserSlice<'a, T, false>> for UserSlice<'a, T, true> {
    fn into(self) -> UserSlice<'a, T, false> {
        UserSlice {
            ptr: self.ptr,
            length: self.length,
            marker: PhantomData,
        }
    }
}

/// Represents an object in user memory.
/// Can be safely constructed from a pointer.
pub type UserPtrMut<'a, T> = UserPtr<'a, T, true>;

/// Represents an object in user memory.
/// Can be safely constructed from a pointer.
#[derive(Clone, Copy)]
pub struct UserPtr<'a, T: UserCopyable, const MUTABLE: bool = false> {
    ptr: NonNull<T>,
    marker: PhantomData<&'a T>,
}

impl<'a, T: UserCopyable> UserPtr<'a, T, false> {
    /// Create a pointer from kernel memory; it is assumed to be safe to access.
    pub fn new_kernel(ptr: &'a T) -> AccessResult<Self> {
        Ok(Self {
            ptr: unsafe { NonNull::new_unchecked(ptr as *const T as *mut T) },
            marker: PhantomData,
        })
    }

    /// Create a new user-access pointer; will validate that the range is user memory.
    pub fn new(ptr: *const T) -> AccessResult<Self> {
        let vaddr = ptr as usize;
        if !mem::vmm::pagetable::is_canon_user_range(
            vaddr..vaddr.checked_add(size_of::<T>()).ok_or(AccessFault)?,
        ) {
            return Err(AccessFault);
        }
        Ok(Self {
            ptr: NonNull::new(ptr as *mut T).ok_or(AccessFault)?,
            marker: PhantomData,
        })
    }

    /// Create a new nullable user-access pointer; will validate that the range is user memory.
    pub fn new_nullable(ptr: *const T) -> AccessResult<Option<Self>> {
        let vaddr = ptr as usize;
        if !mem::vmm::pagetable::is_canon_user_range(
            vaddr..vaddr.checked_add(size_of::<T>()).ok_or(AccessFault)?,
        ) {
            return Err(AccessFault);
        }
        Ok(try {
            Self {
                ptr: NonNull::new(ptr as *mut T)?,
                marker: PhantomData,
            }
        })
    }
}

impl<'a, T: UserCopyable> UserPtr<'a, T, true> {
    /// Create a pointer from kernel memory; it is assumed to be safe to access.
    pub fn new_kernel_mut(ptr: &'a mut T) -> AccessResult<Self> {
        Ok(Self {
            ptr: unsafe { NonNull::new_unchecked(ptr as *const T as *mut T) },
            marker: PhantomData,
        })
    }

    /// Create a new user-access pointer; will validate that the range is user memory.
    pub fn new_mut(ptr: *mut T) -> AccessResult<Self> {
        let vaddr = ptr as usize;
        if !mem::vmm::pagetable::is_canon_user_range(
            vaddr..vaddr.checked_add(size_of::<T>()).ok_or(AccessFault)?,
        ) {
            return Err(AccessFault);
        }
        Ok(Self {
            ptr: NonNull::new(ptr as *mut T).ok_or(AccessFault)?,
            marker: PhantomData,
        })
    }

    /// Create a new nullable user-access pointer; will validate that the range is user memory.
    pub fn new_nullable_mut(ptr: *mut T) -> AccessResult<Option<Self>> {
        let vaddr = ptr as usize;
        if !mem::vmm::pagetable::is_canon_user_range(
            vaddr..vaddr.checked_add(size_of::<T>()).ok_or(AccessFault)?,
        ) {
            return Err(AccessFault);
        }
        Ok(try {
            Self {
                ptr: NonNull::new(ptr as *mut T)?,
                marker: PhantomData,
            }
        })
    }
}

impl<'a, T: UserCopyable, const MUTABLE: bool> UserPtr<'a, T, MUTABLE> {
    /// Try to read an element from the slice.
    pub fn read(&self) -> AccessResult<T> {
        let mut tmp = MaybeUninit::uninit();
        unsafe { cpu::mmu::enable_sum() };
        let faulted = unsafe {
            isr_noexc_mem_copy(
                &raw mut tmp as *mut c_void,
                self.ptr.as_ptr() as *const c_void,
                size_of::<T>(),
            )
        };
        unsafe { cpu::mmu::disable_sum() };
        if faulted {
            Err(AccessFault)
        } else {
            Ok(unsafe { tmp.assume_init() })
        }
    }

    /// Get the pointer.
    pub fn as_ptr(&self) -> *const T {
        self.ptr.as_ptr()
    }
}

impl<'a, T: UserCopyable> UserPtr<'a, T, true> {
    /// Try to write an element to the slice.
    pub fn write(&mut self, data: T) -> AccessResult<()> {
        unsafe { cpu::mmu::enable_sum() };
        let faulted = unsafe {
            isr_noexc_mem_copy(
                self.ptr.as_ptr() as *mut c_void,
                &raw const data as *const c_void,
                size_of::<T>(),
            )
        };
        unsafe { cpu::mmu::disable_sum() };
        if faulted { Err(AccessFault) } else { Ok(()) }
    }

    /// Get the pointer.
    pub fn as_mut_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }
}

/// Read a C-string from user memory into a preallocated buffer.
pub fn read_user_cstr(mut user_cstr: *const c_char, buffer: &mut [u8]) -> AccessResult<usize> {
    unsafe { cpu::mmu::enable_sum() };

    for i in 0..buffer.len() {
        if !mem::vmm::pagetable::is_canon_user_addr(user_cstr as usize) {
            unsafe { cpu::mmu::disable_sum() };
            return Err(AccessFault);
        }
        let mut c = 0;
        let faulted = unsafe { isr_noexc_copy_u8(&raw mut c, user_cstr as *const u8) };
        if faulted {
            unsafe { cpu::mmu::disable_sum() };
            return Err(AccessFault);
        }
        if c == 0 {
            break;
        }
        buffer[i] = c;
        user_cstr = user_cstr.wrapping_add(1);
    }

    unsafe { cpu::mmu::disable_sum() };
    return Ok(buffer.len());
}

/// Read a C-string from user memory into a new [`CString`].
pub fn copy_user_cstr(mut user_cstr: *const c_char) -> EResult<CString> {
    unsafe { cpu::mmu::enable_sum() };
    let mut res = Vec::new();

    loop {
        if !mem::vmm::pagetable::is_canon_user_addr(user_cstr as usize) {
            unsafe { cpu::mmu::disable_sum() };
            return Err(AccessFault);
        }
        let mut c = 0;
        let faulted = unsafe { isr_noexc_copy_u8(&raw mut c, user_cstr as *const u8) };
        if faulted {
            unsafe { cpu::mmu::disable_sum() };
            return Err(AccessFault);
        }
        if c == 0 {
            break;
        }
        res.try_reserve(1)?;
        res.push(c);
        user_cstr = user_cstr.wrapping_add(1);
    }

    unsafe { cpu::mmu::disable_sum() };
    Ok(unsafe { CString::from_vec_unchecked(res) })
}
