// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    arch::asm,
    cell::UnsafeCell,
    mem::offset_of,
    ptr::{NonNull, null_mut, slice_from_raw_parts_mut},
    sync::atomic::{AtomicU32, Ordering, fence},
};

use alloc::{boxed::Box, sync::Arc};

use crate::{
    badgelib::irq::IrqGuard,
    bindings::{error::EResult, log::LogLevel, raw::timestamp_us_t, time_us},
    config::{PAGE_SIZE, STACK_SIZE},
    cpu::{self, cpulocal::ArchCpuLocal, thread::context_switch},
    mem::vmm,
    scheduler::{
        cpulocal::CpuLocal,
        sync::{mutex::RawMutex, rcu::RcuCtx},
    },
    util::list::{ArcList, HasListNode, InvasiveListNode},
};

pub mod cpulocal;
pub mod sync;
pub mod sysimpl;
pub mod waitlist;

/// Dynamic thread runtime state.
struct ThreadRuntime {
    /// Stack bottom.
    stack_bottom: usize,
    /// Current stack pointer.
    stack_ptr: *mut (),
}

impl ThreadRuntime {
    fn new(code: Box<dyn FnOnce() + 'static + Send>) -> EResult<Self> {
        unsafe {
            let stack_vpn = vmm::kernel_mm().map_ram(
                None,
                STACK_SIZE as usize / PAGE_SIZE as usize,
                vmm::flags::RW,
            )?;

            let stack_bottom = stack_vpn * PAGE_SIZE as usize;
            let stack = slice_from_raw_parts_mut(
                stack_bottom as *mut usize,
                STACK_SIZE as usize / size_of::<usize>(),
            );

            let stack_used = cpu::thread::prepare_entry(&mut *stack, code) * size_of::<usize>();
            let stack_ptr = (stack_bottom + STACK_SIZE as usize - stack_used) as *mut ();

            fence(Ordering::Release);

            Ok(Self {
                stack_bottom,
                stack_ptr,
            })
        }
    }
}

impl Drop for ThreadRuntime {
    fn drop(&mut self) {
        unsafe {
            let stack_vpn = self.stack_bottom / PAGE_SIZE as usize;
            vmm::kernel_mm().unmap(stack_vpn..stack_vpn + STACK_SIZE as usize / PAGE_SIZE as usize);
        }
    }
}

pub mod tflags {
    pub const STOPPED: u32 = 1;
    pub const BLOCKED: u32 = 2;
}

/// Thread control block.
pub struct Thread {
    node: InvasiveListNode<Thread>,
    flags: AtomicU32,
    runtime: UnsafeCell<Option<ThreadRuntime>>,
}
impl HasListNode<Thread> for Thread {
    fn list_node(&self) -> &InvasiveListNode<Thread> {
        &self.node
    }

    fn list_node_mut(&mut self) -> &mut InvasiveListNode<Thread> {
        &mut self.node
    }

    unsafe fn from_node(node: &InvasiveListNode<Thread>) -> &Thread {
        unsafe {
            &*((node as *const InvasiveListNode<Thread>).byte_sub(offset_of!(Thread, node))
                as *const Thread)
        }
    }

    unsafe fn from_node_mut(node: &mut InvasiveListNode<Thread>) -> &mut Thread {
        unsafe {
            &mut *((node as *mut InvasiveListNode<Thread>).byte_sub(offset_of!(Thread, node))
                as *mut Thread)
        }
    }
}

impl Thread {
    /// Prepare thread control block but do not add it to a scheduler.
    fn new_tcb_only(code: Box<dyn FnOnce() + 'static + Send>) -> EResult<Arc<Self>> {
        let tcb = Arc::try_new(Thread {
            flags: AtomicU32::new(0),
            node: InvasiveListNode::new(),
            runtime: UnsafeCell::new(Some(ThreadRuntime::new(code)?)),
        })?;

        Ok(tcb)
    }

    /// Create and start a new thread.
    pub fn new_impl(code: Box<dyn FnOnce() + 'static + Send>) -> EResult<Arc<Self>> {
        let tcb = Self::new_tcb_only(code)?;

        unsafe {
            let _noirq = IrqGuard::new();
            let cpulocal = CpuLocal::get().unwrap().as_mut();
            cpulocal
                .sched
                .as_mut()
                .unwrap()
                .queue
                .push_back(tcb.clone())
                .unwrap();
        }

        Ok(tcb)
    }

    /// Create and start a new thread.
    pub fn new(code: impl FnOnce() + 'static + Send) -> EResult<Arc<Self>> {
        Self::new_impl(Box::try_new(code)?)
    }

    /// Get the currently running thread.
    pub fn current() -> Option<NonNull<Thread>> {
        Some(unsafe { NonNull::from(CpuLocal::get()?.as_mut().thread.as_deref()?) })
    }

    /// Terminate the current thread.
    pub unsafe fn die(&self) -> ! {
        self.flags.fetch_or(tflags::STOPPED, Ordering::Relaxed);
        thread_yield();
        unreachable!()
    }
}

/// Number of currently running schedulers.
static RUNNING_SCHED_COUNT: AtomicU32 = AtomicU32::new(0);

/// Instance of a scheduler running on one CPU.
pub struct Scheduler {
    /// Idle thread for this scheduler.
    idle: Option<Arc<Thread>>,
    /// Runnable thread queue.
    queue: ArcList<Thread>,
    /// Threads to reap queue.
    zombies: ArcList<Thread>,
    /// Implements RCU semantics.
    rcu: RcuCtx,
}

impl Scheduler {
    pub fn new() -> EResult<Self> {
        let idle = Thread::new_tcb_only(Box::try_new(|| Self::idle_func())?)?;

        Ok(Self {
            idle: Some(idle),
            queue: ArcList::new(),
            zombies: ArcList::new(),
            rcu: RcuCtx::new(),
        })
    }

    /// Scheduler idle function.
    fn idle_func() -> ! {
        loop {
            thread_yield();
        }
    }

    /// Start this scheduler on the local CPU.
    pub unsafe fn exec(&mut self) -> ! {
        RUNNING_SCHED_COUNT.fetch_add(1, Ordering::Relaxed);
        self.rcu.post_start_callback();
        self.reschedule();
        unreachable!();
    }

    /// Choose the next thread to run and remove it from the queue.
    fn choose_thread(&mut self) -> Option<Arc<Thread>> {
        for _ in 0..self.queue.len() {
            let node = self.queue.pop_front().unwrap();
            let flags = node.flags.load(Ordering::Relaxed);

            if flags & tflags::STOPPED != 0 {
                self.zombies.push_back(node).unwrap();
                continue;
            } else if flags & tflags::BLOCKED == 0 {
                return Some(node);
            }

            self.queue.push_back(node).unwrap();
        }
        None
    }

    /// Yield the current thread's execution.
    fn reschedule(&mut self) {
        // TODO: Time accounting.
        self.rcu.sched_callback();
        unsafe {
            let cpulocal = CpuLocal::get().unwrap().as_mut();
            let mut old = None;
            core::mem::swap(&mut old, &mut cpulocal.thread);

            let mut dummy = null_mut();
            let old_stack_out: *mut *mut ();
            if let Some(old) = old {
                old_stack_out = &raw mut old.runtime.as_mut_unchecked().as_mut().unwrap().stack_ptr;
                if self.idle.is_none() {
                    self.idle = Some(old);
                } else {
                    self.queue.push_back(old).unwrap();
                }
            } else {
                old_stack_out = &raw mut dummy;
            }

            let next = self.choose_thread().unwrap_or_else(|| {
                let mut next = None;
                core::mem::swap(&mut next, &mut self.idle);
                next.unwrap()
            });

            let new_stack = next.runtime.as_ref_unchecked().as_ref().unwrap().stack_ptr;
            cpulocal.thread = Some(next);
            context_switch(new_stack, old_stack_out);
        }
    }
}

/// Yield the current thread's execution.
pub fn thread_yield() {
    unsafe {
        CpuLocal::get()
            .unwrap()
            .as_mut()
            .sched
            .as_mut()
            .unwrap()
            .reschedule();
    }
}

/// Sleep for a fixed amount of time.
/// Only fails if interrupted by a signal.
pub fn thread_sleep(amount: timestamp_us_t) -> EResult<()> {
    let ts = time_us() + amount;
    while time_us() < ts {
        thread_yield();
    }
    Ok(())
}
