// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

use crate::{
    bindings::{error::EResult, raw::timestamp_us_t},
    kernel::sync::waitlist::Waitlist,
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
    /// May fail with [`crate::bindings::error::Errno::EINTR`] if signalled.
    pub fn wait(&self) -> EResult<()> {
        self.timed_wait(timestamp_us_t::MAX)
    }

    /// Await one post from the semaphore.
    /// May fail with [`crate::bindings::error::Errno::EINTR`] if signalled.
    pub fn timed_wait(&self, timeout: timestamp_us_t) -> EResult<()> {
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
        self.unintr_timed_wait(timestamp_us_t::MAX)
    }

    /// Await one post from the semaphore.
    pub fn unintr_timed_wait(&self, timeout: timestamp_us_t) {
        // Fast path.
        for _ in 0..50 {
            if self
                .counter
                .try_update(Ordering::Release, Ordering::Relaxed, |x| x.checked_sub(1))
                .is_ok()
            {
                return;
            }
        }

        // Slow path.
        while !self
            .counter
            .try_update(Ordering::Release, Ordering::Relaxed, |x| x.checked_sub(1))
            .is_ok()
        {
            self.waitlist
                .unintr_block(timeout, || self.counter.load(Ordering::Relaxed) == 0);
        }
    }
}

mod c_api {
    use crate::bindings::{
        error::Errno,
        raw::{errno_t, timestamp_us_t},
    };

    use super::Semaphore;

    #[unsafe(no_mangle)]
    extern "C" fn sem_post(sem: &Semaphore) {
        sem.post();
    }

    #[unsafe(no_mangle)]
    extern "C" fn sem_wait(sem: &Semaphore) -> errno_t {
        Errno::extract(sem.wait())
    }

    #[unsafe(no_mangle)]
    extern "C" fn sem_timed_wait(sem: &Semaphore, timeout: timestamp_us_t) -> errno_t {
        Errno::extract(sem.timed_wait(timeout))
    }
}
