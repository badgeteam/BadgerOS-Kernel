// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

use crate::{
    bindings::{error::EResult, raw::timestamp_us_t},
    kernel::waitlist::Waitlist,
};

/// A counting semaphore.
#[repr(C)]
#[derive(Debug)]
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
