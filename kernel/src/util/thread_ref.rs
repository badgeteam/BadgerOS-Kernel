// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

/// Simple reference wrapper that is not [`Send`].
/// Represents a reference with a thread-local lifetime.
#[derive(Clone, Copy)]
pub struct ThreadRef<T> {
    ptr: NonNull<T>,
}

impl<T> ThreadRef<T> {
    pub const unsafe fn new(ptr: NonNull<T>) -> Self {
        Self { ptr }
    }

    pub fn map<'a, U, F>(self, f: F) -> ThreadRef<U>
    where
        T: 'a,
        U: 'a,
        F: FnOnce(&'a T) -> &'a U,
    {
        ThreadRef {
            ptr: unsafe { NonNull::new_unchecked(f(&*self.ptr.as_ptr()) as *const U as *mut U) },
        }
    }

    pub fn try_map<'a, U, F>(self, f: F) -> Option<ThreadRef<U>>
    where
        T: 'a,
        U: 'a,
        F: FnOnce(&'a T) -> Option<&'a U>,
    {
        Some(ThreadRef {
            ptr: unsafe { NonNull::new_unchecked(f(&*self.ptr.as_ptr())? as *const U as *mut U) },
        })
    }
}

impl<T> !Sync for ThreadRef<T> {}

impl<T> Deref for ThreadRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr.as_ptr() }
    }
}

/// Mutable version of [`ThreadRef`].
pub struct ThreadMut<T> {
    ptr: NonNull<T>,
}

impl<T> ThreadMut<T> {
    pub const unsafe fn new(ptr: NonNull<T>) -> Self {
        Self { ptr }
    }

    pub fn map<'a, U, F>(self, f: F) -> ThreadMut<U>
    where
        T: 'a,
        U: 'a,
        F: FnOnce(&'a mut T) -> &'a mut U,
    {
        ThreadMut {
            ptr: unsafe { NonNull::new_unchecked(f(&mut *self.ptr.as_ptr()) as *mut U) },
        }
    }

    pub fn try_map<'a, U, F>(self, f: F) -> Option<ThreadMut<U>>
    where
        T: 'a,
        U: 'a,
        F: FnOnce(&'a mut T) -> Option<&'a mut U>,
    {
        Some(ThreadMut {
            ptr: unsafe { NonNull::new_unchecked(f(&mut *self.ptr.as_ptr())? as *mut U) },
        })
    }
}

impl<T> !Sync for ThreadMut<T> {}

impl<T> Deref for ThreadMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr.as_ptr() }
    }
}

impl<T> DerefMut for ThreadMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr.as_ptr() }
    }
}

impl<T> Into<ThreadRef<T>> for ThreadMut<T> {
    fn into(self) -> ThreadRef<T> {
        ThreadRef { ptr: self.ptr }
    }
}
