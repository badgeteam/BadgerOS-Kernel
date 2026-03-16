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
    pub const fn new() -> Self {
        Self {
            waitlist: Waitlist::new(),
            counter: AtomicU32::new(0),
        }
    }

    /// Post once to the semaphore.
    pub fn post(&self) {
        self.counter.fetch_add(1, Ordering::Release);
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
                .try_update(Ordering::Release, Ordering::Relaxed, |x| {
                    (x > 0).then_some(x - 1)
                })
                .is_ok()
            {
                return Ok(());
            }
        }

        // Slow path.
        while !self
            .counter
            .try_update(Ordering::Release, Ordering::Relaxed, |x| {
                (x > 0).then_some(x - 1)
            })
            .is_ok()
        {
            self.waitlist
                .block(timeout, || self.counter.load(Ordering::Relaxed) == 0)?;
        }

        Ok(())
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
