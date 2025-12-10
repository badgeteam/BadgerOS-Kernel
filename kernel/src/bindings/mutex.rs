use core::{
    cell::UnsafeCell,
    fmt::Debug,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

use super::{
    error::{EResult, Errno},
    raw::{self, mutex_t, timestamp_us_t},
};

/// A BadgerOS mutex with a value in it.
#[repr(C)]
#[derive(Debug)]
pub struct Mutex<T, const SHARED: bool = true> {
    inner: UnsafeCell<mutex_t>,
    data: UnsafeCell<T>,
}
unsafe impl<T, const SHARED: bool> Send for Mutex<T, SHARED> {}
unsafe impl<T, const SHARED: bool> Sync for Mutex<T, SHARED> {}

impl<T, const SHARED: bool> Mutex<T, SHARED> {
    pub unsafe fn mutex(&self) -> &mut mutex_t {
        unsafe { self.inner.as_mut_unchecked() }
    }

    pub unsafe fn data(&self) -> &mut T {
        unsafe { self.data.as_mut_unchecked() }
    }

    /// Create a new mutex statically; you must only use this to initialize the `.data` section.
    pub const unsafe fn new_static(data: T) -> Self {
        Self {
            inner: UnsafeCell::new(mutex_t {
                is_shared: SHARED,
                shares: 0,
                waiting_list: unsafe { core::mem::zeroed() },
            }),
            data: UnsafeCell::new(data),
        }
    }

    /// Create a new mutex.
    pub fn new(data: T) -> Self {
        let mut inner: mutex_t = unsafe { MaybeUninit::zeroed().assume_init() };
        unsafe { raw::mutex_init(&mut inner, SHARED) };
        Self {
            inner: UnsafeCell::new(inner),
            data: UnsafeCell::new(data),
        }
    }

    /// Try to lock the mutex.
    pub fn try_lock<'a>(&'a self, timeout: timestamp_us_t) -> EResult<MutexGuard<'a, T, SHARED>> {
        MutexGuard::try_new(self, timeout)
    }

    /// Lock the mutex.
    pub fn lock<'a>(&'a self) -> MutexGuard<'a, T, SHARED> {
        MutexGuard::new(self)
    }
}

impl<T> Mutex<T, true> {
    /// Try to lock the mutex as shared.
    pub fn try_lock_shared<'a>(
        &'a self,
        timeout: timestamp_us_t,
    ) -> EResult<SharedMutexGuard<'a, T>> {
        SharedMutexGuard::try_new(self, timeout)
    }

    /// Lock the mutex as shared.
    pub fn lock_shared<'a>(&'a self) -> SharedMutexGuard<'a, T> {
        SharedMutexGuard::new(self)
    }
}

impl<T, const SHARED: bool> Mutex<T, SHARED> {
    /// Write the value in the mutex.
    pub fn write(&self, value: T) {
        *self.lock() = value
    }
}

impl<T: Clone> Mutex<T, true> {
    /// Read the value in the mutex.
    pub fn read(&self) -> T {
        (*self.lock_shared()).clone()
    }
}

impl<T: Clone> Mutex<T, false> {
    /// Read the value in the mutex.
    pub fn read(&self) -> T {
        self.lock().clone()
    }
}

impl<T: Clone> Clone for Mutex<T, true> {
    fn clone(&self) -> Self {
        Self::new((*self.lock_shared()).clone())
    }
}

impl<T: Clone> Clone for Mutex<T, false> {
    fn clone(&self) -> Self {
        Self::new(self.lock().clone())
    }
}

/// Represents a locked mutex.
pub struct MutexGuard<'a, T: ?Sized, const SHARED: bool> {
    mutex: *mut mutex_t,
    data: *mut T,
    marker: PhantomData<&'a mutex_t>,
}

impl<'a, T: ?Sized + 'a, const SHARED: bool> MutexGuard<'a, T, SHARED> {
    /// Convert into raw parts.
    pub fn to_raw_parts(self) -> (*mut mutex_t, &'a mut T) {
        // Using this unsafe block to bypass borrow checker from preventing the `core::mem::forget`.
        let tmp = unsafe { (self.mutex, &mut *(self.data as *mut T)) };
        core::mem::forget(self);
        tmp
    }
    /// Convert this guard into one of a different type referencing the same mutex.
    pub fn convert<U: 'a>(
        self,
        f: impl FnOnce(&'a mut T) -> &'a mut U,
    ) -> MutexGuard<'a, U, SHARED> {
        let (mutex, data) = self.to_raw_parts();
        let res = MutexGuard {
            mutex,
            data: f(data),
            marker: PhantomData,
        };
        res
    }
    /// Create from an already locked mutex and a data pointer.
    pub unsafe fn from_raw_parts(mutex: *mut mutex_t, data: &'a mut T) -> Self {
        Self {
            mutex,
            data,
            marker: PhantomData,
        }
    }
    /// Try to lock a mutex.
    pub unsafe fn try_new_raw(
        mutex: *mut mutex_t,
        data: *mut T,
        timeout: timestamp_us_t,
    ) -> EResult<Self> {
        unsafe {
            raw::mutex_acquire(mutex, timeout)
                .then_some(Self {
                    mutex,
                    data,
                    marker: PhantomData {},
                })
                .ok_or(Errno::ETIMEDOUT)
        }
    }
    /// Lock a mutex.
    pub unsafe fn new_raw(mutex: *mut mutex_t, data: *mut T) -> Self {
        unsafe { Self::try_new_raw(mutex, data, timestamp_us_t::MAX) }.unwrap()
    }
}

impl<'a, T: Sized, const SHARED: bool> MutexGuard<'a, T, SHARED> {
    /// Try to lock a mutex.
    pub fn try_new(mutex: &'a Mutex<T, SHARED>, timeout: timestamp_us_t) -> EResult<Self> {
        unsafe {
            raw::mutex_acquire(mutex.mutex(), timeout)
                .then_some(Self {
                    mutex: mutex.mutex(),
                    data: mutex.data(),
                    marker: PhantomData {},
                })
                .ok_or(Errno::ETIMEDOUT)
        }
    }
    /// Lock a mutex.
    pub fn new(mutex: &'a Mutex<T, SHARED>) -> Self {
        Self::try_new(mutex, timestamp_us_t::MAX).unwrap()
    }
}

impl<'a, T: ?Sized, const SHARED: bool> Drop for MutexGuard<'a, T, SHARED> {
    fn drop(&mut self) {
        unsafe { raw::mutex_release(self.mutex) };
    }
}

impl<T: ?Sized, const SHARED: bool> Deref for MutexGuard<'_, T, SHARED> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.data }
    }
}

impl<T: ?Sized, const SHARED: bool> DerefMut for MutexGuard<'_, T, SHARED> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.data }
    }
}

/// Represents a mutex locked shared.
pub struct SharedMutexGuard<'a, T: ?Sized> {
    mutex: *mut mutex_t,
    data: *const T,
    marker: PhantomData<&'a mutex_t>,
}

impl<'a, T: ?Sized + 'a> SharedMutexGuard<'a, T> {
    /// Convert into raw parts.
    pub fn to_raw_parts(self) -> (*mut mutex_t, &'a T) {
        // Using this unsafe block to bypass borrow checker from preventing the `core::mem::forget`.
        let tmp = unsafe { (self.mutex, &*(self.data as *const T)) };
        core::mem::forget(self);
        tmp
    }
    /// Create a clone of this mutex guard.
    pub fn share(&self) -> Self {
        unsafe { raw::mutex_acquire_shared(self.mutex, timestamp_us_t::MAX) };
        Self {
            mutex: self.mutex,
            data: self.data,
            marker: PhantomData,
        }
    }
    /// Convert this guard into one of a different type referencing the same mutex.
    pub fn convert<U: 'a>(self, f: impl FnOnce(&'a T) -> &'a U) -> SharedMutexGuard<'a, U> {
        let (mutex, data) = self.to_raw_parts();
        let res = SharedMutexGuard {
            mutex,
            data: f(data),
            marker: PhantomData,
        };
        res
    }
    /// Create from an already locked mutex and a data pointer.
    pub unsafe fn from_raw_parts(mutex: *mut mutex_t, data: *const T) -> Self {
        Self {
            mutex,
            data,
            marker: PhantomData,
        }
    }
    /// Try to lock a mutex as shared.
    pub unsafe fn try_new_raw(
        mutex: *mut mutex_t,
        data: *const T,
        timeout: timestamp_us_t,
    ) -> EResult<Self> {
        unsafe {
            raw::mutex_acquire_shared(mutex, timeout)
                .then_some(Self {
                    mutex,
                    data,
                    marker: PhantomData {},
                })
                .ok_or(Errno::ETIMEDOUT)
        }
    }
    /// Lock a mutex as shared.
    pub unsafe fn new_raw(mutex: *mut mutex_t, data: *const T) -> Self {
        unsafe { Self::try_new_raw(mutex, data, timestamp_us_t::MAX) }.unwrap()
    }
}

impl<'a, T: Sized> SharedMutexGuard<'a, T> {
    /// Try to lock a mutex as shared.
    pub fn try_new(mutex: &'a Mutex<T, true>, timeout: timestamp_us_t) -> EResult<Self> {
        unsafe {
            raw::mutex_acquire_shared(mutex.mutex(), timeout)
                .then_some(Self {
                    mutex: mutex.mutex(),
                    data: mutex.data(),
                    marker: PhantomData {},
                })
                .ok_or(Errno::ETIMEDOUT)
        }
    }
    /// Lock a mutex as shared.
    pub fn new(mutex: &'a Mutex<T, true>) -> Self {
        Self::try_new(mutex, timestamp_us_t::MAX).unwrap()
    }
}

impl<'a, T: ?Sized> Drop for SharedMutexGuard<'a, T> {
    fn drop(&mut self) {
        unsafe { raw::mutex_release_shared(self.mutex) };
    }
}

impl<'a, T: ?Sized> Deref for SharedMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.data }
    }
}
