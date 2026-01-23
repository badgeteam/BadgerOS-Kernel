// SPDX-FileCopyrightText: 2025-2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    mem::offset_of,
    ptr::{null_mut, slice_from_raw_parts_mut},
    sync::atomic::{AtomicI64, AtomicU32, Ordering, fence},
};

use alloc::{boxed::Box, string::String, sync::Arc};

use crate::{
    badgelib::irq::IrqGuard,
    bindings::{error::EResult, log::LogLevel, raw::timestamp_us_t, time_us},
    config::{self, PAGE_SIZE, STACK_SIZE},
    cpu::{self, thread::context_switch, usermode::ThreadUContext},
    impl_has_list_node,
    kernel::{
        cpulocal::CpuLocal,
        smp,
        sync::{rcu::RcuCtx, spinlock::Spinlock, waitlist::Waitlist},
    },
    mem::vmm::{self, Memmap, kernel_mm},
    misc::panic,
    process::Process,
    util::list::{ArcList, HasListNode, InvasiveListNode},
};

/// Dynamic thread runtime state.
pub struct ThreadRuntime {
    /// Stack bottom.
    pub stack_bottom: usize,
    /// Current stack pointer.
    pub stack_ptr: *mut (),
    /// Stack pointer to use for interrupts.
    pub irq_stack: *mut (),
    /// Context for running in userspace.
    pub uctx: ThreadUContext,
    /// Timestamp until which to keep the thread blocked.
    pub timeout: timestamp_us_t,
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
                irq_stack: null_mut(),
                stack_bottom,
                stack_ptr,
                uctx: ThreadUContext::default(),
                timeout: 0,
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
    /// Thread is no longer running.
    pub const STOPPED: u32 = 1 << 0;
    /// Request for thread to stop running (causes termination of user-mode code).
    pub const STOPPING: u32 = 1 << 1;
    /// Thread is blocked on a synchronization object.
    pub const BLOCKED: u32 = 1 << 2;
    /// Thread blocking is interruptible by signals.
    pub const SIGNALABLE: u32 = 1 << 3;
}

/// Thread control block.
pub struct Thread {
    node: InvasiveListNode<Thread>,
    /// Flags about the blocking status, lifetime, etc.
    flags: AtomicU32,
    /// Dynamic state only alive while the thread is runnable.
    runtime: UnsafeCell<Option<ThreadRuntime>>,
    /// How many microseconds of CPU time this thread has spent in kernel mode.
    ktime: AtomicI64,
    /// How many microseconds of CPU time this thread has spent in user mode.
    utime: AtomicI64,
    /// Waitlist for objects blocking on a state update of this thread.
    pub waitlist: Waitlist,
    /// Process with which this thread is associated.
    pub process: Option<Arc<Process>>,
    /// Thread name for debugging purposes.
    pub name: Option<String>,
}
impl_has_list_node!(Thread, node);

impl Thread {
    /// Prepare thread control block but do not add it to a scheduler.
    fn new_tcb_only(
        code: Box<dyn FnOnce() + 'static + Send>,
        process: Option<Arc<Process>>,
        name: Option<String>,
    ) -> EResult<Arc<Self>> {
        let tcb = Arc::try_new(Thread {
            flags: AtomicU32::new(0),
            node: InvasiveListNode::new(),
            runtime: UnsafeCell::new(Some(ThreadRuntime::new(code)?)),
            ktime: AtomicI64::new(0),
            utime: AtomicI64::new(0),
            waitlist: Waitlist::new(),
            process,
            name,
        })?;

        Ok(tcb)
    }

    /// Create and start a new thread.
    pub fn new_impl(
        code: Box<dyn FnOnce() + 'static + Send>,
        process: Option<Arc<Process>>,
        name: Option<String>,
    ) -> EResult<Arc<Self>> {
        let tcb = Self::new_tcb_only(code, process, name)?;

        unsafe {
            let _noirq = IrqGuard::new();
            let cpulocal = &mut *CpuLocal::get();
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
    pub fn new(
        code: impl FnOnce() + 'static + Send,
        process: Option<Arc<Process>>,
        name: Option<String>,
    ) -> EResult<Arc<Self>> {
        Self::new_impl(Box::try_new(code)?, process, name)
    }

    /// Get the currently running thread.
    pub fn current() -> *mut Thread {
        unsafe {
            let cpulocal = CpuLocal::get();
            if cpulocal.is_null() {
                return null_mut();
            }
            if let Some(thread) = &(*cpulocal).thread {
                thread.as_ref() as *const Thread as *mut Thread
            } else {
                null_mut()
            }
        }
    }

    /// Set the STOPPING flag, asking for this thread to be stopped.
    pub fn stop(&self) {
        self.flags.fetch_or(tflags::STOPPING, Ordering::Relaxed);
    }

    /// Test whether the STOPPING flag is set.
    pub fn is_stopping(&self) -> bool {
        self.flags.load(Ordering::Relaxed) & tflags::STOPPING != 0
    }

    /// Terminate the current thread.
    pub unsafe fn die(&self) -> ! {
        self.flags.fetch_or(tflags::STOPPED, Ordering::Relaxed);
        thread_yield();
        unreachable!()
    }

    /// Wait for this thread to stop.
    pub fn join(&self) -> EResult<()> {
        while self.flags.load(Ordering::Relaxed) & tflags::STOPPED == 0 {
            self.waitlist.block(timestamp_us_t::MAX, || {
                self.flags.load(Ordering::Relaxed) & tflags::STOPPED == 0
            })?;
        }
        Ok(())
    }

    pub unsafe fn runtime(&self) -> &mut ThreadRuntime {
        unsafe { self.runtime.as_mut_unchecked().as_mut().unwrap() }
    }
}

/// Number of currently running schedulers.
pub(super) static RUNNING_SCHED_COUNT: AtomicU32 = AtomicU32::new(0);
/// Global list of threads to be reaped.
static ZOMBIES: Spinlock<ArcList<Thread>> = Spinlock::new(ArcList::new());
/// The thread reaper thread.
static REAPER: Spinlock<Option<Arc<Thread>>> = Spinlock::new(None);

/// Instance of a scheduler running on one CPU.
pub struct Scheduler {
    /// Idle thread for this scheduler.
    idle: Option<Arc<Thread>>,
    /// Runnable thread queue.
    queue: ArcList<Thread>,
    /// Implements RCU semantics.
    rcu: RcuCtx,
    /// How many ticks until the current thread is preempted.
    /// If set to 0, the thread will not be preempted.
    preempt_ticks: u32,
}

impl Scheduler {
    pub fn new() -> EResult<Self> {
        let mut queue = ArcList::new();

        // Idle power management and work stealing.
        let idle = Thread::new_tcb_only(
            Box::try_new(|| Self::idle_func())?,
            None,
            Some(format!("Idle for CPU{}", smp::cur_cpu())),
        )?;

        // Reports that the CPU is up; should ideally be the first in the queue.
        let up_reporter = Thread::new_tcb_only(
            Box::try_new(|| smp::report_online())?,
            None,
            Some(format!("Up-reporter for CPU{}", smp::cur_cpu())),
        )?;
        let _ = queue.push_front(up_reporter);

        {
            let _noirq = IrqGuard::new();
            let mut reaper = REAPER.lock();
            if reaper.is_none() {
                // Reaps the control blocks of dead threads.
                let tcb = Thread::new_tcb_only(
                    Box::new(|| Self::reaper_func()),
                    None,
                    Some("Threads reaper".into()),
                )?;
                *reaper = Some(tcb.clone());
                let _ = queue.push_back(tcb);
            }
        }

        Ok(Self {
            idle: Some(idle),
            queue,
            rcu: RcuCtx::new(),
            preempt_ticks: 0,
        })
    }

    /// Thread reaper function.
    fn reaper_func() -> ! {
        let thread_self = unsafe { &*Thread::current() };
        loop {
            thread_yield();

            // Threads are transferred to this list and dropped at the end of the loop.
            let mut threads = ArcList::new();

            let _noirq = IrqGuard::new();

            thread_self
                .flags
                .fetch_or(tflags::BLOCKED, Ordering::Relaxed);

            let mut zombies = ZOMBIES.lock();
            core::mem::swap(&mut threads, &mut *zombies);
            drop(zombies);

            if threads.len() != 0 {
                thread_self
                    .flags
                    .fetch_and(!tflags::BLOCKED, Ordering::Relaxed);
            }

            logkf!(LogLevel::Debug, "Reaping {} threads", threads.len());
        }
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
        cpu::timer::start_tick_timer();
        self.reschedule();
        unreachable!();
    }

    /// Choose the next thread to run and remove it from the queue.
    fn choose_thread(&mut self) -> Option<Arc<Thread>> {
        for _ in 0..self.queue.len() {
            let node = self.queue.pop_front().unwrap();
            let flags = node.flags.load(Ordering::Relaxed);

            if flags & tflags::STOPPED != 0 {
                ZOMBIES.lock().push_back(node).unwrap();
                REAPER
                    .lock_shared()
                    .as_deref()
                    .unwrap()
                    .flags
                    .fetch_and(!tflags::BLOCKED, Ordering::Relaxed);
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
        debug_assert!(!cpu::irq::is_enabled());
        self.rcu.sched_callback();
        unsafe {
            let cpulocal = &mut *CpuLocal::get();
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

            self.preempt_ticks = (config::TICKS_PER_SEC as u32).div_ceil(100);

            // Switch to next page table.
            let new_mm: &Memmap;
            if let Some(process) = next.process.as_ref() {
                new_mm = process.memmap();
            } else {
                new_mm = kernel_mm();
            }
            cpu::mmu::set_page_table(new_mm.root_ppn(), 0);
            cpu::mmu::vmem_fence(None, None);

            cpulocal.arch.set_irq_stack(next.runtime().irq_stack);
            let new_stack = &raw const next.runtime().stack_ptr;
            cpulocal.thread = Some(next);
            context_switch(new_stack, old_stack_out);
        }
    }

    /// Account current thread's time usage since it was last accounted.
    /// If `as_user_time` is true, it is counted as userspace time.
    /// Otherwise, it is counted as kernel time.
    pub fn account_time(&mut self, as_user_time: bool) {}

    /// Called every time a timer tick interrupt happens.
    /// Accounts thread time usage and manages preemption.
    /// If `as_user_time` is true, it is counted as userspace time.
    /// Otherwise, it is counted as kernel time.
    pub fn tick_interrupt(&mut self, is_user_time: bool) {
        self.account_time(is_user_time);
        cpu::timer::start_tick_timer();
        panic::check_for_panic();

        if self.preempt_ticks > 0 {
            self.preempt_ticks -= 1;
            if self.preempt_ticks == 0 {
                self.reschedule();
            }
        }
    }

    pub fn get() -> *mut Self {
        unsafe { &mut *CpuLocal::get() }.sched.as_mut().unwrap()
    }
}

/// Yield the current thread's execution.
#[unsafe(no_mangle)]
pub extern "C" fn thread_yield() {
    unsafe {
        let _noirq = IrqGuard::new();
        if let Some(sched) = (*CpuLocal::get()).sched.as_mut() {
            sched.reschedule();
        }
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

mod c_api {
    use crate::bindings::{
        error::Errno,
        raw::{errno_t, timestamp_us_t},
    };

    #[unsafe(no_mangle)]
    extern "C" fn thread_sleep(amount: timestamp_us_t) -> errno_t {
        Errno::extract(super::thread_sleep(amount))
    }
}
