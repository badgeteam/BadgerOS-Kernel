// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::Ordering;

use crate::{
    bindings::{
        error::{EResult, Errno},
        raw::timestamp_us_t,
    },
    impl_has_list_node,
    kernel::sched::{Thread, tflags, thread_yield},
    util::list::{InvasiveList, InvasiveListNode},
};

use super::spinlock::Spinlock;

/// Entry in the waitiling list.
#[repr(C)]
struct WaitingTicket {
    node: InvasiveListNode,
    thread: *const Thread,
}
impl_has_list_node!(WaitingTicket, node);

/// Helper struct used to construct types that block threads.
#[repr(C)]
pub struct Waitlist {
    list: Spinlock<InvasiveList<WaitingTicket>>,
}

impl Waitlist {
    pub const fn new() -> Self {
        Waitlist {
            list: Spinlock::new(InvasiveList::new()),
        }
    }

    /// Block on this list if a condition is met.
    /// May spuriously return early.
    pub fn unintr_block(&self, timeout: timestamp_us_t, condition: impl FnOnce() -> bool) {
        self.block_impl(timeout, condition);
    }

    /// Block on this list if a condition is met.
    /// May spuriously return early, or return [`Errno::EINTR`] if the thread was signalled.
    pub fn block(&self, timeout: timestamp_us_t, condition: impl FnOnce() -> bool) -> EResult<()> {
        let current = unsafe { &*Thread::current() };
        self.block_impl(timeout, || {
            condition() && unsafe { current.get_async_sig(true).is_none() }
        });
        if unsafe { current.get_async_sig(true).is_some() } {
            Err(Errno::EINTR)
        } else {
            Ok(())
        }
    }

    /// Implementation of [`Self::block`] and [`Self::unintr_block`].
    fn block_impl(&self, timeout: timestamp_us_t, condition: impl FnOnce() -> bool) {
        unsafe {
            let current = { &*Thread::current() };
            current.runtime().timeout = timeout;
            current.flags.fetch_or(tflags::BLOCKED, Ordering::Relaxed);

            let mut ticket = WaitingTicket {
                node: InvasiveListNode::new(),
                thread: current,
            };
            let _ = self.list.lock().push_back(&raw mut ticket);

            if condition() {
                thread_yield();
                // BLOCKED flag will be cleared by the scheduler here.
            } else {
                current.flags.fetch_and(!tflags::BLOCKED, Ordering::Relaxed);
            }

            let _ = self.list.lock().try_remove(&raw mut ticket);
        }
    }

    /// Notify at least one thread on this list.
    pub fn notify(&self) {
        unsafe {
            let mut list = self.list.lock();
            if let Some(ticket) = list.pop_front() {
                (&*(&*ticket).thread)
                    .flags
                    .fetch_and(!tflags::BLOCKED, Ordering::Relaxed);
            }
        }
    }

    /// Notify all threads on this list.
    pub fn notify_all(&self) {
        unsafe {
            let mut list = self.list.lock();
            for ticket in list.iter() {
                (&*(&*ticket).thread)
                    .flags
                    .fetch_and(!tflags::BLOCKED, Ordering::Relaxed);
            }
            list.clear();
        }
    }
}
