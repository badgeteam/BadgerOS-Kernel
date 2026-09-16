// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

use crate::{
    error::{EResult, Errno},
    kcore::{sync::waitlist::Waitlist, timer::time_us},
};

/// A counting semaphore.
#[repr(C)]
pub struct Semaphore {
    waitlist: Waitlist,
    counter: AtomicU32,
}

impl Semaphore {
    /// How many posts can occur before it is considered suspiciously large and probably a bug.
    pub const POST_LIMIT: u32 = 65536;

    pub const fn new() -> Self {
        Self {
            waitlist: Waitlist::new(),
            counter: AtomicU32::new(0),
        }
    }

    pub const fn with_count(initial_counter: u32) -> Self {
        debug_assert!(
            initial_counter < Self::POST_LIMIT,
            "Suspiciously high initial Semaphore counter"
        );
        Self {
            waitlist: Waitlist::new(),
            counter: AtomicU32::new(initial_counter),
        }
    }

    /// Reset the counter.
    pub fn reset(&self) {
        self.counter.store(0, Ordering::Relaxed);
    }

    /// Post once to the semaphore.
    pub fn post(&self) {
        let tmp = self.counter.fetch_add(1, Ordering::Release);
        debug_assert!(
            tmp < Self::POST_LIMIT,
            "Suspiciously high Semaphore counter"
        );
        self.waitlist.notify();
    }

    /// Await one post from the semaphore.
    /// May fail with [`crate::error::Errno::EINTR`] if signalled.
    pub fn wait(&self) -> EResult<()> {
        self.timed_wait(u64::MAX)
    }

    /// Await one post from the semaphore.
    /// May fail with [`crate::error::Errno::EINTR`] if signalled.
    pub fn timed_wait(&self, timeout: u64) -> EResult<()> {
        // Fast path.
        for _ in 0..50 {
            if self
                .counter
                .try_update(Ordering::Release, Ordering::Relaxed, |x| x.checked_sub(1))
                .is_ok()
            {
                return Ok(());
            }
        }

        // Slow path.
        while !self
            .counter
            .try_update(Ordering::Release, Ordering::Relaxed, |x| x.checked_sub(1))
            .is_ok()
        {
            self.waitlist
                .block(timeout, || self.counter.load(Ordering::Relaxed) == 0)?;
        }

        Ok(())
    }

    /// Await one post from the semaphore.
    pub fn unintr_wait(&self) {
        self.unintr_timed_wait(u64::MAX).unwrap();
    }

    /// Await one post from the semaphore.
    pub fn unintr_timed_wait(&self, timeout: u64) -> EResult<()> {
        let lim = time_us().saturating_add(timeout);

        // Fast path.
        for _ in 0..50 {
            if self
                .counter
                .try_update(Ordering::Release, Ordering::Relaxed, |x| x.checked_sub(1))
                .is_ok()
            {
                return Ok(());
            }
        }

        // Slow path.
        while !self
            .counter
            .try_update(Ordering::Release, Ordering::Relaxed, |x| x.checked_sub(1))
            .is_ok()
        {
            let now = time_us();
            self.waitlist.unintr_block(lim.saturating_sub(now), || {
                self.counter.load(Ordering::Relaxed) == 0
            });
            if now > lim {
                return Err(Errno::ETIMEDOUT);
            }
        }

        Ok(())
    }
}
