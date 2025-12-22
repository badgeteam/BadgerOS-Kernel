// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU32, Ordering},
    u32,
};

use crate::{bindings::error::EResult, scheduler::waitlist::Waitlist};

/// Raw mutually-exclusive resource access guard.
#[repr(C)]
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
        RawMutexGuard::new(self)
    }

    pub fn lock_shared<'a>(&'a self) -> EResult<SharedRawMutexGuard<'a>> {
        SharedRawMutexGuard::new(self)
    }
}

/// Exclusive access held to a [`RawMutex`].
pub struct RawMutexGuard<'a> {
    mutex: &'a RawMutex,
}

impl<'a> RawMutexGuard<'a> {
    pub fn new(mutex: &'a RawMutex) -> EResult<Self> {
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
                .block(|| mutex.shares.load(Ordering::Relaxed) != 0)?;
        }

        Ok(Self { mutex })
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
    pub fn new(mutex: &'a RawMutex) -> EResult<Self> {
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
                        .block(|| mutex.shares.load(Ordering::Relaxed) != 0)?;
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
        self.mutex.shares.store(0, Ordering::Release);
        self.mutex.waitlist.notify();
    }
}

/// Mutex-protected resource.
#[repr(C)]
pub struct Mutex<T: Sized> {
    inner: RawMutex,
    data: UnsafeCell<T>,
}

impl<T: Sized> Mutex<T> {
    pub const fn new(data: T) -> Self {
        Self {
            inner: RawMutex::new(),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock<'a>(&'a self) -> EResult<MutexGuard<'a, T>> {
        MutexGuard::new(self)
    }

    pub fn lock_shared<'a>(&'a self) -> EResult<SharedMutexGuard<'a, T>> {
        SharedMutexGuard::new(self)
    }
}

/// Exclusive access held to a [`Mutex`].
pub struct MutexGuard<'a, T> {
    inner: RawMutexGuard<'a>,
    data: &'a mut T,
}

impl<'a, T> MutexGuard<'a, T> {
    pub fn new(mutex: &'a Mutex<T>) -> EResult<Self> {
        Ok(Self {
            inner: mutex.inner.lock()?,
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
    pub fn new(mutex: &'a Mutex<T>) -> EResult<Self> {
        Ok(Self {
            inner: mutex.inner.lock_shared()?,
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
}

impl<T> Deref for SharedMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}
