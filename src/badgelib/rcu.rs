// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::{alloc::AllocError, boxed::Box};

use crate::bindings::{self, isr_ctx::isr_ctx_get};
use core::{
    marker::PhantomData,
    sync::atomic::{AtomicPtr, Ordering},
};

use super::irq::IrqGuard;

/// Run code in an RCU critical section.
macro_rules! rcu_crititcal {
    ($($rcu_guard: ident,)? {$($code: tt)*}) => {
        let noirq = unsafe { crate::badgelib::irq::IrqGuard::new() };
        let rcu_guard = unsafe { crate::badgelib::rcu::RcuGuard::new(&noirq) };
        $(let $rcu_guard = &rcu_guard;)?

        { $($code)* }
    }
}

/// RCU-pointer box.
pub struct Rcu<T> {
    pointer: AtomicPtr<T>,
}

impl<T> From<Box<T>> for Rcu<T> {
    fn from(value: Box<T>) -> Self {
        Self {
            pointer: AtomicPtr::new(Box::into_raw(value)),
        }
    }
}

impl<T> Rcu<T> {
    /// Create a new RCU pointer.
    #[inline(always)]
    pub fn new(data: T) -> Result<Self, AllocError> {
        Ok(Box::try_new(data)?.into())
    }

    /// Get the pointer from an RCU critical section.
    #[inline(always)]
    pub fn get<'a>(&self, _guard: &'a RcuGuard) -> &'a T {
        unsafe { &*self.pointer.load(Ordering::Acquire) }
    }

    /// Exchange the pointer with a new one.
    pub fn exchange(&self, new_data: Box<T>) -> Box<T> {
        let pointer = self.pointer.swap(Box::into_raw(new_data), Ordering::AcqRel);
        rcu_sync();
        unsafe { Box::from_raw(pointer) }
    }

    /// Write new data into the RCU pointer.
    pub fn write(&self, new_data: Box<T>) {
        drop(self.exchange(new_data));
    }
}

impl<T: Clone> Rcu<T> {
    /// Read the data behind the RCU pointer.
    #[inline(always)]
    pub fn read(&self) -> T {
        rcu_crititcal! {rcu_guard, { self.get(rcu_guard).clone() }}
    }
}

/// RCU critical section guard.
pub struct RcuGuard {
    /// Prevents construction and escape to another thread.
    inner: PhantomData<*const ()>,
}

impl RcuGuard {
    /// Create a new RCU guard.
    /// # Safety
    /// It is unsafe to return an RCU guard from a function and interrupts must be disabled.
    #[inline(always)]
    pub unsafe fn new(_guard: &IrqGuard) -> Self {
        // Debug: Mark as in RCU critical section.
        #[cfg(debug_assertions)]
        let isr_ctx = isr_ctx_get();
        #[cfg(debug_assertions)]
        unsafe {
            (*(*isr_ctx).cpulocal).rcu.depth += 1
        };

        Self { inner: PhantomData }
    }
}

impl Drop for RcuGuard {
    fn drop(&mut self) {
        // Debug: Exit RCU critical section.
        #[cfg(debug_assertions)]
        let isr_ctx = isr_ctx_get();
        #[cfg(debug_assertions)]
        unsafe {
            (*(*isr_ctx).cpulocal).rcu.depth -= 1
        };
    }
}

/// Synchronize RCU for reclamation.
pub fn rcu_sync() {
    unsafe {
        bindings::raw::rcu_sync();
    }
}
