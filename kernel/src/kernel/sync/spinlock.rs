// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut, FromResidual, Try},
    sync::atomic::{AtomicU32, Ordering},
};

/// Simple synchronization primitive which spins in a loop until successfully acquiring the lock.
pub struct RawSpinlock {
    shares: AtomicU32,
}
unsafe impl Send for RawSpinlock {}
unsafe impl Sync for RawSpinlock {}

impl RawSpinlock {
    pub const fn new() -> Self {
        Self {
            shares: AtomicU32::new(0),
        }
    }

    pub fn lock<'a>(&'a self) -> RawSpinlockGuard<'a> {
        RawSpinlockGuard::new(self)
    }

    pub fn lock_shared(&'_ self) -> SharedRawSpinlockGuard<'_> {
        SharedRawSpinlockGuard::new(self)
    }

    pub fn is_locked(&self) -> bool {
        self.shares.load(Ordering::Relaxed) == u32::MAX
    }
}

/// Represents an exclusively-held [`RawSpinlock`].
pub struct RawSpinlockGuard<'a> {
    lock: &'a RawSpinlock,
}

impl<'a> RawSpinlockGuard<'a> {
    pub unsafe fn from_raw(lock: &'a RawSpinlock) -> Self {
        Self { lock }
    }

    pub fn new(lock: &'a RawSpinlock) -> Self {
        while lock
            .shares
            .compare_exchange_weak(0, u32::MAX, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {}
        Self { lock }
    }

    pub fn demote(self) -> SharedRawSpinlockGuard<'a> {
        let lock: &'a RawSpinlock = unsafe { core::mem::transmute(self) };
        lock.shares.store(1, Ordering::Release);
        SharedRawSpinlockGuard { lock }
    }
}

impl<'a> Drop for RawSpinlockGuard<'a> {
    fn drop(&mut self) {
        self.lock.shares.store(0, Ordering::Release);
    }
}

/// Represents a non-exclusively-held [`RawSpinlock`].
pub struct SharedRawSpinlockGuard<'a> {
    lock: &'a RawSpinlock,
}

impl<'a> SharedRawSpinlockGuard<'a> {
    pub unsafe fn from_raw(lock: &'a RawSpinlock) -> Self {
        Self { lock }
    }

    pub fn new(lock: &'a RawSpinlock) -> Self {
        let mut old = lock.shares.load(Ordering::Relaxed);
        loop {
            if old == u32::MAX {
                old = lock.shares.load(Ordering::Relaxed);
                continue;
            }
            match lock.shares.compare_exchange_weak(
                old,
                old + 1,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Self { lock },
                Err(x) => old = x,
            }
        }
    }

    pub fn share(&self) -> Self {
        let count = self.lock.shares.fetch_add(1, Ordering::Relaxed);
        assert!(count < u32::MAX);
        Self { lock: self.lock }
    }
}

impl<'a> Drop for SharedRawSpinlockGuard<'a> {
    fn drop(&mut self) {
        self.lock.shares.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Simple synchronization primitive which spins in a loop until successfully acquiring the lock.
pub struct Spinlock<T> {
    pub inner: RawSpinlock,
    data: UnsafeCell<T>,
}
unsafe impl<T> Send for Spinlock<T> {}
unsafe impl<T> Sync for Spinlock<T> {}

impl<T> Spinlock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            inner: RawSpinlock::new(),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&'_ self) -> SpinlockGuard<'_, T> {
        let raw = self.inner.lock();
        SpinlockGuard {
            inner: raw,
            data: unsafe { self.data.as_mut_unchecked() },
        }
    }

    pub fn lock_shared(&'_ self) -> SharedSpinlockGuard<'_, T> {
        let raw = SharedRawSpinlockGuard::new(&self.inner);
        SharedSpinlockGuard {
            inner: raw,
            data: unsafe { self.data.as_ref_unchecked() },
        }
    }

    pub fn is_locked(&self) -> bool {
        self.inner.is_locked()
    }
}

/// Represents an exclusively-held [`Spinlock`].
pub struct SpinlockGuard<'a, T> {
    inner: RawSpinlockGuard<'a>,
    data: &'a mut T,
}

impl<'a, T> SpinlockGuard<'a, T> {
    pub unsafe fn from_raw(inner: RawSpinlockGuard<'a>, data: &'a mut T) -> Self {
        Self { inner, data }
    }

    pub fn into_raw(self) -> RawSpinlockGuard<'a> {
        self.inner
    }

    pub fn demote(self) -> SharedSpinlockGuard<'a, T> {
        SharedSpinlockGuard {
            inner: self.inner.demote(),
            data: self.data,
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

    pub fn convert<U: 'a>(self, f: impl FnOnce(&'a mut T) -> &'a mut U) -> SpinlockGuard<'a, U> {
        SpinlockGuard {
            inner: self.inner,
            data: f(self.data),
        }
    }

    /// Like [`Self::convert`], but for [`Try`] types.
    /// For example, creating [`Option<SpinlockGuard<U>>`] from [`SpinlockGuard<Option<U>>`].
    pub fn try_convert<
        U: 'a,
        V: Try<Output = &'a mut U>,
        W: Try<Output = SpinlockGuard<'a, U>> + FromResidual<V::Residual>,
        F: FnOnce(&'a mut T) -> V,
    >(
        self,
        f: F,
    ) -> W {
        W::from_output(SpinlockGuard {
            inner: self.inner,
            data: f(self.data)?,
        })
    }
}

impl<'a, T> Deref for SpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T> DerefMut for SpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

/// Represents a non-exclusively-held [`Spinlock`].
pub struct SharedSpinlockGuard<'a, T> {
    inner: SharedRawSpinlockGuard<'a>,
    data: &'a T,
}

impl<'a, T> SharedSpinlockGuard<'a, T> {
    pub unsafe fn from_raw(inner: SharedRawSpinlockGuard<'a>, data: &'a T) -> Self {
        Self { inner, data }
    }

    pub fn into_raw(self) -> SharedRawSpinlockGuard<'a> {
        self.inner
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

    pub fn convert<U: 'a>(self, f: impl FnOnce(&'a T) -> &'a U) -> SharedSpinlockGuard<'a, U> {
        SharedSpinlockGuard {
            inner: self.inner,
            data: f(self.data),
        }
    }

    /// Like [`Self::convert`], but for [`Try`] types.
    /// For example, creating [`Option<SharedSpinlockGuard<U>>`] from [`SharedSpinlockGuard<Option<U>>`].
    pub fn try_convert<
        U: 'a,
        V: Try<Output = &'a U>,
        W: Try<Output = SharedSpinlockGuard<'a, U>> + FromResidual<V::Residual>,
        F: FnOnce(&'a T) -> V,
    >(
        self,
        f: F,
    ) -> W {
        W::from_output(SharedSpinlockGuard {
            inner: self.inner,
            data: f(self.data)?,
        })
    }
}

impl<'a, T> Deref for SharedSpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}
