// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::Ordering;

use alloc::{boxed::Box, vec::Vec};

use crate::{
    badgelib::irq::IrqGuard,
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
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
            let _noirq = IrqGuard::new();
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
            let _noirq = IrqGuard::new();
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
            let _noirq = IrqGuard::new();
            let mut list = self.list.lock();
            for ticket in list.iter() {
                (&*(&*ticket).thread)
                    .flags
                    .fetch_and(!tflags::BLOCKED, Ordering::Relaxed);
            }
            list.clear();
        }
    }

    /// Implementation of [`Self::select`] and [`Self::unintr_select`].
    /// Can only fail if `interruptible` and a signal is pending on this thread.
    fn select_impl(
        timeout: timestamp_us_t,
        lists: &[Waitlist],
        tickets: &mut [WaitingTicket],
        checks: Vec<Box<dyn FnOnce() -> bool>>,
        interruptible: bool,
    ) -> EResult<()> {
        unsafe {
            let mut res = Ok(());
            let _noirq = IrqGuard::new();
            let current = { &*Thread::current() };
            current.runtime().timeout = timeout;
            current.flags.fetch_or(tflags::BLOCKED, Ordering::Relaxed);

            let mut block = true;
            for (i, check) in checks.into_iter().enumerate() {
                let _ = lists[i].list.lock().push_front(&raw mut tickets[i]);
                if !check() {
                    block = false;
                    break;
                }
            }
            if block && interruptible && current.get_async_sig(true).is_some() {
                block = false;
                res = Err(Errno::EINTR);
            }

            if block {
                thread_yield();
            } else {
                current.flags.fetch_and(!tflags::BLOCKED, Ordering::Relaxed);
            }

            for i in 0..lists.len() {
                // Note: Would fail if hadn't been inserted, but this can happen if one of the checks fails, so continue anyway.
                let _ = lists[i].list.lock().try_remove(&raw mut tickets[i]);
            }

            res
        }
    }

    /// Wait for one of any number of events to happen.
    /// The same waitlist may occur multiple times, but there is no real reason to do so.
    pub fn unintr_select(
        timeout: timestamp_us_t,
        lists: &[Waitlist],
        checks: Vec<Box<dyn FnOnce() -> bool>>,
    ) -> EResult<()> {
        if lists.len() != checks.len() {
            logkf!(
                LogLevel::Error,
                "Waistlist::unintr_select len mismatch between lists({}) and checks({})",
                lists.len(),
                checks.len()
            );
            return Err(Errno::EINVAL);
        }
        let current = unsafe { &*Thread::current() };

        let mut tickets = Vec::try_with_capacity(lists.len())?;
        tickets.resize_with(lists.len(), || WaitingTicket {
            node: InvasiveListNode::new(),
            thread: current,
        });

        // Can't fail because not interruptible.
        let _ = Self::select_impl(timeout, lists, &mut tickets, checks, false);

        Ok(())
    }

    /// Wait for one of any number of events to happen.
    /// The same waitlist may occur multiple times, but there is no real reason to do so.
    pub fn select(
        timeout: timestamp_us_t,
        lists: &[Waitlist],
        checks: Vec<Box<dyn FnOnce() -> bool>>,
    ) -> EResult<()> {
        if lists.len() != checks.len() {
            logkf!(
                LogLevel::Error,
                "Waistlist::select len mismatch between lists({}) and checks({})",
                lists.len(),
                checks.len()
            );
            return Err(Errno::EINVAL);
        }
        let current = unsafe { &*Thread::current() };

        let mut tickets = Vec::try_with_capacity(lists.len())?;
        tickets.resize_with(lists.len(), || WaitingTicket {
            node: InvasiveListNode::new(),
            thread: current,
        });

        Self::select_impl(timeout, lists, &mut tickets, checks, false)
    }
}
