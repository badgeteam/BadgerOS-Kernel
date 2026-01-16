// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{
    bindings::{error::EResult, raw::timestamp_us_t},
    kernel::sched::thread_yield,
};

/// Helper struct used to construct types that block threads.
#[repr(C)]
#[derive(Debug)]
pub struct Waitlist {
    marker: (),
}

impl Waitlist {
    pub const fn new() -> Self {
        Waitlist { marker: () }
    }

    /// Block on this list if a condition is met.
    /// May spuriously return early.
    pub fn unintr_block(&self, timeout: timestamp_us_t, condition: impl FnOnce() -> bool) {
        // TODO: Currently a no-op.
        let _ = condition();
        let _ = timeout;
        thread_yield();
    }

    /// Block on this list if a condition is met.
    /// May spuriously return early, or return [`Errno::EINTR`] if the thread was signalled.
    #[inline(always)]
    pub fn block(&self, timeout: timestamp_us_t, condition: impl FnOnce() -> bool) -> EResult<()> {
        // TODO: Currently a no-op.
        // TODO: Check for pending signals.
        let _ = condition();
        let _ = timeout;
        thread_yield();
        Ok(())
    }

    /// Notify at least one thread on this list.
    pub fn notify(&self) {}

    /// Notify all threads on this list.
    pub fn notify_all(&self) {}
}
