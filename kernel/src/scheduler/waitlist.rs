// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{bindings::error::EResult, scheduler::thread_yield};

/// Helper struct used to construct types that block threads.
#[repr(C)]
pub struct Waitlist {}

impl Waitlist {
    pub const fn new() -> Self {
        Waitlist {}
    }

    /// Block on this list if a condition is met.
    /// May spuriously return early, or return [`Errno::EINTR`] if the thread was signalled.
    #[inline(always)]
    pub fn block(&self, condition: impl FnOnce() -> bool) -> EResult<()> {
        // TODO: Currently a no-op.
        // TODO: Check for pending signals.
        let _ = condition();
        thread_yield();
        Ok(())
    }

    /// Notify at least one thread on this list.
    pub fn notify(&self) {}

    /// Notify all threads on this list.
    pub fn notify_all(&self) {}
}
