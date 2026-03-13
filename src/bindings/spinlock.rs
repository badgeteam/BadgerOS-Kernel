use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::atomic::{Ordering, fence},
};

use super::raw::{self, spinlock_t};

/// A BadgerOS spinlock with a value in it.
#[repr(C)]
pub struct Spinlock<T, const SHARED: bool = true> {
    inner: UnsafeCell<spinlock_t>,
    data: UnsafeCell<T>,
}
unsafe impl<T, const SHARED: bool> Send for Spinlock<T, SHARED> {}
unsafe impl<T, const SHARED: bool> Sync for Spinlock<T, SHARED> {}

impl<T, const SHARED: bool> Spinlock<T, SHARED> {
    pub unsafe fn spinlock(&self) -> &mut spinlock_t {
        unsafe { self.inner.as_mut_unchecked() }
    }

    pub unsafe fn data(&self) -> &mut T {
        unsafe { self.data.as_mut_unchecked() }
    }
}

impl<T, const SHARED: bool> Spinlock<T, SHARED> {
    /// Create a new spinlock statically; you must only use this to initialize the `.data` section.
    pub const unsafe fn new_static(data: T) -> Self {
        Self {
            inner: UnsafeCell::new(SHARED as spinlock_t),
            data: UnsafeCell::new(data),
        }
    }
    /// Create a new spinlock.
    pub fn new(data: T) -> Self {
        let inner = UnsafeCell::new(SHARED as spinlock_t);
        fence(Ordering::Release);
        Self {
            inner,
            data: UnsafeCell::new(data),
        }
    }
}

impl<T, const SHARED: bool> Spinlock<T, SHARED> {
    /// Lock the spinlock.
    pub fn lock<'a>(&'a self) -> SpinlockGuard<'a, T, SHARED> {
        SpinlockGuard::new(self)
    }
}

impl<T> Spinlock<T, true> {
    /// Lock the spinlock as shared.
    pub fn lock_shared<'a>(&'a self) -> SharedSpinlockGuard<'a, T> {
        SharedSpinlockGuard::new(self)
    }
}

impl<T, const SHARED: bool> Spinlock<T, SHARED> {
    /// Write the value in the spinlock.
    pub fn write(&self, value: T) {
        *self.lock() = value
    }
}

impl<T: Clone> Spinlock<T, true> {
    /// Read the value in the spinlock.
    pub fn read(&self) -> T {
        self.lock_shared().clone()
    }
}

impl<T: Clone> Spinlock<T, false> {
    /// Read the value in the spinlock.
    pub fn read(&self) -> T {
        self.lock().clone()
    }
}

/// Represents a locked spinlock.
pub struct SpinlockGuard<'a, T, const SHARED: bool> {
    spinlock: *mut spinlock_t,
    data: &'a mut T,
    marker: PhantomData<&'a Spinlock<T, SHARED>>,
}

impl<'a, T, const SHARED: bool> SpinlockGuard<'a, T, SHARED> {
    /// Lock a spinlock.
    pub unsafe fn new_raw(spinlock: *mut spinlock_t, data: &'a mut T) -> Self {
        unsafe { raw::spinlock_take(spinlock) };
        Self {
            spinlock,
            data,
            marker: PhantomData,
        }
    }
    /// Lock a spinlock.
    pub fn new(spinlock: &'a Spinlock<T, SHARED>) -> Self {
        unsafe {
            raw::spinlock_take(spinlock.spinlock());
            Self {
                spinlock: spinlock.spinlock(),
                data: spinlock.data(),
                marker: PhantomData,
            }
        }
    }
}

impl<'a, T, const SHARED: bool> Drop for SpinlockGuard<'a, T, SHARED> {
    fn drop(&mut self) {
        unsafe { raw::spinlock_release(self.spinlock) };
    }
}

impl<T, const SHARED: bool> Deref for SpinlockGuard<'_, T, SHARED> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T, const SHARED: bool> DerefMut for SpinlockGuard<'_, T, SHARED> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

/// Represents a spinlock locked shared.
pub struct SharedSpinlockGuard<'a, T> {
    spinlock: *mut spinlock_t,
    data: &'a T,
    marker: PhantomData<&'a Spinlock<T, true>>,
}

impl<'a, T> SharedSpinlockGuard<'a, T> {
    /// Lock a spinlock as shared.
    pub unsafe fn new_raw(spinlock: *mut spinlock_t, data: &'a T) -> Self {
        unsafe { raw::spinlock_take_shared(spinlock) };
        Self {
            spinlock,
            data,
            marker: PhantomData,
        }
    }
    /// Lock a spinlock as shared.
    pub fn new(spinlock: &'a Spinlock<T, true>) -> Self {
        unsafe {
            raw::spinlock_take_shared(spinlock.spinlock());
            Self {
                spinlock: spinlock.spinlock(),
                data: spinlock.data(),
                marker: PhantomData,
            }
        }
    }
}

impl<'a, T> Deref for SharedSpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}
