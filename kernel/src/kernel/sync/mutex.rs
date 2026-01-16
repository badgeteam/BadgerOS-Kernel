// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU32, Ordering},
    u32,
};

use crate::{
    bindings::{
        error::{EResult, Errno},
        raw::{errno_t, timestamp_us_t},
    },
    kernel::sync::waitlist::Waitlist,
};

/// Raw mutually-exclusive resource access guard.
#[repr(C)]
#[derive(Debug)]
pub struct RawMutex {
    waitlist: Waitlist,
    shares: AtomicU32,
}

impl RawMutex {
    pub const fn new() -> Self {
        Self {
            waitlist: Waitlist::new(),
            shares: AtomicU32::new(0),
        }
    }

    pub fn lock<'a>(&'a self) -> EResult<RawMutexGuard<'a>> {
        RawMutexGuard::new(self, timestamp_us_t::MAX)
    }

    pub fn timed_lock<'a>(&'a self, timeout: timestamp_us_t) -> EResult<RawMutexGuard<'a>> {
        RawMutexGuard::new(self, timeout)
    }

    pub fn lock_shared<'a>(&'a self) -> EResult<SharedRawMutexGuard<'a>> {
        SharedRawMutexGuard::new(self, timestamp_us_t::MAX)
    }

    pub fn timed_lock_shared<'a>(
        &'a self,
        timeout: timestamp_us_t,
    ) -> EResult<SharedRawMutexGuard<'a>> {
        SharedRawMutexGuard::new(self, timeout)
    }

    /// Version of [`Self::lock`] that can't be interrupted.
    pub fn unintr_lock<'a>(&'a self) -> RawMutexGuard<'a> {
        // TODO: Can't *actually* be interrupted yet because of no signals being implemented.
        self.lock().unwrap()
    }

    /// Version of [`Self::lock_shared`] that can't be interrupted.
    pub fn unintr_lock_shared<'a>(&'a self) -> SharedRawMutexGuard<'a> {
        // TODO: Can't *actually* be interrupted yet because of no signals being implemented.
        self.lock_shared().unwrap()
    }
}

/// Exclusive access held to a [`RawMutex`].
#[repr(transparent)]
pub struct RawMutexGuard<'a> {
    mutex: &'a RawMutex,
}

impl<'a> RawMutexGuard<'a> {
    fn new(mutex: &'a RawMutex, timeout: timestamp_us_t) -> EResult<Self> {
        // Fast path.
        for _ in 0..50 {
            if mutex
                .shares
                .compare_exchange_weak(0, u32::MAX, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return Ok(Self { mutex });
            }
        }

        // Slow path.
        while !mutex
            .shares
            .compare_exchange_weak(0, u32::MAX, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            mutex
                .waitlist
                .block(timeout, || mutex.shares.load(Ordering::Relaxed) != 0)?;
        }

        Ok(Self { mutex })
    }

    pub fn demote(self) -> SharedRawMutexGuard<'a> {
        let mutex: &'a RawMutex = unsafe { core::mem::transmute(self) };
        mutex.shares.store(1, Ordering::Release);
        mutex.waitlist.notify_all();
        SharedRawMutexGuard { mutex }
    }
}

impl<'a> Drop for RawMutexGuard<'a> {
    fn drop(&mut self) {
        self.mutex.shares.store(0, Ordering::Release);
        self.mutex.waitlist.notify_all();
    }
}

/// Shared access held to a [`RawMutex`].
pub struct SharedRawMutexGuard<'a> {
    mutex: &'a RawMutex,
}

impl<'a> SharedRawMutexGuard<'a> {
    fn new(mutex: &'a RawMutex, timeout: timestamp_us_t) -> EResult<Self> {
        // Fast path.
        let mut old = mutex.shares.load(Ordering::Relaxed);
        for _ in 0..50 {
            if old == u32::MAX {
                old = mutex.shares.load(Ordering::Relaxed);
                continue;
            }
            match mutex.shares.compare_exchange_weak(
                old,
                old + 1,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(Self { mutex }),
                Err(x) => old = x,
            }
        }

        // Slow path.
        loop {
            if old == u32::MAX {
                old = mutex.shares.load(Ordering::Relaxed);
                continue;
            }
            match mutex.shares.compare_exchange_weak(
                old,
                old + 1,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(Self { mutex }),
                Err(x) => {
                    old = x;
                    mutex
                        .waitlist
                        .block(timeout, || mutex.shares.load(Ordering::Relaxed) != 0)?;
                }
            }
        }
    }

    pub fn share(&self) -> Self {
        let count = self.mutex.shares.fetch_add(1, Ordering::Relaxed);
        assert!(count < u32::MAX);
        Self { mutex: self.mutex }
    }
}

impl<'a> Drop for SharedRawMutexGuard<'a> {
    fn drop(&mut self) {
        if self.mutex.shares.fetch_sub(1, Ordering::Release) == 1 {
            self.mutex.waitlist.notify();
        }
    }
}

/// Mutex-protected resource.
#[repr(C)]
#[derive(Debug)]
pub struct Mutex<T> {
    inner: RawMutex,
    data: UnsafeCell<T>,
}
unsafe impl<T> Send for Mutex<T> {}
unsafe impl<T> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    pub const fn new(data: T) -> Self {
        Self {
            inner: RawMutex::new(),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock<'a>(&'a self) -> EResult<MutexGuard<'a, T>> {
        MutexGuard::new(self, timestamp_us_t::MAX)
    }

    pub fn timed_lock<'a>(&'a self, timeout: timestamp_us_t) -> EResult<MutexGuard<'a, T>> {
        MutexGuard::new(self, timeout)
    }

    pub fn lock_shared<'a>(&'a self) -> EResult<SharedMutexGuard<'a, T>> {
        SharedMutexGuard::new(self, timestamp_us_t::MAX)
    }

    pub fn timed_lock_shared<'a>(
        &'a self,
        timeout: timestamp_us_t,
    ) -> EResult<SharedMutexGuard<'a, T>> {
        SharedMutexGuard::new(self, timeout)
    }

    pub unsafe fn data(&self) -> &mut T {
        unsafe { self.data.as_mut_unchecked() }
    }

    /// Version of [`Self::lock`] that can't be interrupted.
    pub fn unintr_lock<'a>(&'a self) -> MutexGuard<'a, T> {
        // TODO: Can't *actually* be interrupted yet because of no signals being implemented.
        self.lock().unwrap()
    }

    /// Version of [`Self::lock_shared`] that can't be interrupted.
    pub fn unintr_lock_shared<'a>(&'a self) -> SharedMutexGuard<'a, T> {
        // TODO: Can't *actually* be interrupted yet because of no signals being implemented.
        self.lock_shared().unwrap()
    }
}

/// Exclusive access held to a [`Mutex`].
pub struct MutexGuard<'a, T> {
    inner: RawMutexGuard<'a>,
    data: &'a mut T,
}

impl<'a, T> MutexGuard<'a, T> {
    pub unsafe fn from_raw(inner: RawMutexGuard<'a>, data: *mut T) -> Self {
        unsafe {
            Self {
                inner,
                data: &mut *data,
            }
        }
    }

    fn new(mutex: &'a Mutex<T>, timeout: timestamp_us_t) -> EResult<Self> {
        Ok(Self {
            inner: mutex.inner.timed_lock(timeout)?,
            data: unsafe { mutex.data.as_mut_unchecked() },
        })
    }

    pub fn map<U>(self, f: impl FnOnce(&'a mut T) -> &'a mut U) -> MutexGuard<'a, U> {
        MutexGuard {
            inner: self.inner,
            data: f(self.data),
        }
    }

    pub fn read(&self) -> T
    where
        T: Clone,
    {
        self.data.clone()
    }

    pub fn write(&mut self, value: T) {
        *self.data = value
    }

    pub fn convert<U: 'a>(self, f: impl FnOnce(&'a mut T) -> &'a mut U) -> MutexGuard<'a, U> {
        MutexGuard {
            inner: self.inner,
            data: f(self.data),
        }
    }

    pub fn demote(self) -> SharedMutexGuard<'a, T> {
        SharedMutexGuard {
            inner: self.inner.demote(),
            data: self.data,
        }
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

/// Shared access held to a [`Mutex`].
pub struct SharedMutexGuard<'a, T> {
    inner: SharedRawMutexGuard<'a>,
    data: &'a T,
}

impl<'a, T> SharedMutexGuard<'a, T> {
    pub unsafe fn from_raw(inner: SharedRawMutexGuard<'a>, data: *const T) -> Self {
        unsafe {
            Self {
                inner,
                data: &*data,
            }
        }
    }

    fn new(mutex: &'a Mutex<T>, timeout: timestamp_us_t) -> EResult<Self> {
        Ok(Self {
            inner: mutex.inner.timed_lock_shared(timeout)?,
            data: unsafe { mutex.data.as_ref_unchecked() },
        })
    }

    pub fn map<U>(self, f: impl FnOnce(&'a T) -> &'a U) -> SharedMutexGuard<'a, U> {
        SharedMutexGuard {
            inner: self.inner,
            data: f(self.data),
        }
    }

    pub fn share(&self) -> Self {
        Self {
            inner: self.inner.share(),
            data: self.data,
        }
    }

    pub fn read(&self) -> T
    where
        T: Clone,
    {
        self.data.clone()
    }

    pub fn convert<U: 'a>(self, f: impl FnOnce(&'a T) -> &'a U) -> SharedMutexGuard<'a, U> {
        SharedMutexGuard {
            inner: self.inner,
            data: f(self.data),
        }
    }
}

impl<T> Deref for SharedMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn mutex_lock(mutex: &RawMutex) -> errno_t {
    Errno::extract(try { core::mem::forget(mutex.lock()?) })
}

#[unsafe(no_mangle)]
unsafe extern "C" fn mutex_lock_shared(mutex: &RawMutex) -> errno_t {
    Errno::extract(try { core::mem::forget(mutex.lock_shared()?) })
}

#[unsafe(no_mangle)]
unsafe extern "C" fn mutex_timed_lock(mutex: &RawMutex, timeout: timestamp_us_t) -> errno_t {
    Errno::extract(try { core::mem::forget(mutex.timed_lock(timeout)?) })
}

#[unsafe(no_mangle)]
unsafe extern "C" fn mutex_timed_lock_shared(mutex: &RawMutex, timeout: timestamp_us_t) -> errno_t {
    Errno::extract(try { core::mem::forget(mutex.timed_lock_shared(timeout)?) })
}

#[unsafe(no_mangle)]
unsafe extern "C" fn mutex_unlock(mutex: &RawMutex) {
    drop(RawMutexGuard { mutex })
}

#[unsafe(no_mangle)]
unsafe extern "C" fn mutex_unlock_shared(mutex: &RawMutex) {
    drop(SharedRawMutexGuard { mutex })
}
