// SPDX-FileCopyrightText: 2025-2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    ptr::{null, null_mut, slice_from_raw_parts_mut},
    sync::atomic::{AtomicI32, AtomicI64, AtomicU32, Ordering, fence},
};

use alloc::{boxed::Box, collections::linked_list::LinkedList, string::String, sync::Arc};

use crate::{
    badgelib::irq::IrqGuard,
    bindings::{error::EResult, raw::timestamp_us_t, time_us},
    config::{self, STACK_SIZE},
    cpu::{
        self, irq,
        thread::{FloatState, context_switch, pause_hint},
        usermode::ThreadUContext,
    },
    impl_has_list_node,
    kernel::{
        cpulocal::CpuLocal,
        smp,
        sync::{
            rcu::RcuCtx,
            spinlock::{RawSpinlockGuard, Spinlock},
            waitlist::Waitlist,
        },
    },
    mem::vmm::{self, map::VmSpace},
    misc::panic,
    process::{
        Process,
        uapi::{
            signal::{Signal, siginfo_t, stack_t},
            sigset::sigset_t,
        },
    },
    util::{
        bitset::BitSet,
        list::{ArcInvasiveList, InvasiveListNode},
    },
};

mod rr;

/// Thread queue(s) and scheduling algorithm.
trait SchedAlgorithm {
    /// Notify the algorithm that a thread has yielded and may be chosen again.
    fn return_thread(&mut self, thread: Arc<Thread>);
    /// Remove a thread from the queue.
    fn remove_thread(&mut self, thread: Arc<Thread>);
    /// Add a thread to the queue.
    fn add_thread(&mut self, thread: Arc<Thread>);
    /// Add a thread to the front of the queue.
    fn add_thread_front(&mut self, thread: Arc<Thread>);
    /// Choose the next thread that should run.
    /// The same thread may not be chosen again until it is returned via [`SchedAlgorithm::return_thread`].
    fn choose_thread(&mut self) -> Option<Arc<Thread>>;
    /// How many threads are in the queue.
    fn len(&self) -> usize;
}

type ActiveSchedAlgorithm = rr::RoundRobinAlgorithm;

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
    /// Float and/or vector state.
    pub fstate: FloatState,
    /// Alternate signal stack.
    pub sigaltstack: stack_t,
    /// Masked signals; anything in this set delivered asynchronously will be ignored.
    pub sigprocmask: sigset_t,
    /// Current memory map, or null() if default.
    pub memmap: *const VmSpace,
}

impl ThreadRuntime {
    fn new(code: Box<dyn FnOnce() + 'static + Send>) -> EResult<Self> {
        unsafe {
            let stack_bottom = vmm::kernel_mm().map(
                STACK_SIZE as usize,
                0,
                0,
                vmm::prot::READ | vmm::prot::WRITE,
                None,
            )?;

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
                fstate: FloatState::new(),
                sigaltstack: stack_t::default(),
                sigprocmask: sigset_t::default(),
                memmap: null(),
            })
        }
    }
}

impl Drop for ThreadRuntime {
    fn drop(&mut self) {
        unsafe {
            vmm::kernel_mm()
                .unmap(self.stack_bottom..self.stack_bottom + STACK_SIZE as usize)
                .expect("Unmapping kernel stack failed");
        }
    }
}

pub mod tflags {
    /// Thread is no longer runnable.
    pub const STOPPED: u32 = 1 << 0;
    /// Request for thread to stop running (causes termination of user-mode code).
    pub const STOPPING: u32 = 1 << 1;
    /// Thread is blocked on a synchronization object.
    pub const BLOCKED: u32 = 1 << 2;
    /// Thread blocking is interruptible by signals.
    pub const SIGNALABLE: u32 = 1 << 3;
    /// Thread is currently using the CPU.
    pub const RUNNING: u32 = 1 << 4;
}

/// Thread control block.
pub struct Thread {
    node: InvasiveListNode,
    /// Flags about the blocking status, lifetime, etc.
    pub(super) flags: AtomicU32,
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
    /// Queued asynchronous signals.
    /// **WARNING:** Despite being a spinlock, this may NOT be acquired from interrupts.
    pub sigqueue: Spinlock<LinkedList<siginfo_t>>,
}
impl_has_list_node!(Thread, node);

impl Thread {
    /// Part 2: Reconstruct and call the `Box<dyn FnOnce()>`.
    pub unsafe extern "C" fn thread_trampoline_2(
        sched: *const Scheduler,
        ptr: *mut (),
        meta: *mut (),
    ) {
        unsafe {
            let code: *mut dyn FnOnce() =
                core::ptr::from_raw_parts_mut(ptr, core::mem::transmute(meta));
            fence(Ordering::Acquire);
            drop(RawSpinlockGuard::from_raw(&(&*sched).queue.inner()));
            irq::enable();
            Box::from_raw(code)();
            (*Thread::current()).die();
        }
    }

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
            sigqueue: Spinlock::new(LinkedList::new()),
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
                .lock()
                .add_thread(tcb.clone());
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
    pub fn current() -> *const Thread {
        unsafe {
            let _noirq = IrqGuard::new();
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
        self.waitlist.notify_all();
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

    /// Get the first unblocked signal in the async signal queue.
    pub unsafe fn get_async_sig(&self, is_poll: bool) -> Option<siginfo_t> {
        let mask = unsafe { &self.runtime().sigprocmask };
        let mut queue = self.sigqueue.lock();

        // First get signals for this thread specifically.
        let mut iter = queue.cursor_front_mut();
        while let Some(info) = iter.current() {
            if info.si_signo == Signal::SIGSTOP as i32
                || info.si_signo == Signal::SIGKILL as i32
                || !mask.test(info.si_signo as usize)
            {
                let tmp = *info;
                if !is_poll {
                    iter.remove_current();
                }
                return Some(tmp);
            }
            iter.move_next();
        }

        drop(queue);
        if let Some(proc) = self.process.clone() {
            let mut queue = proc.sigqueue.unintr_lock();

            // The get signals for the process in general.
            let mut iter = queue.cursor_front_mut();
            while let Some(info) = iter.current() {
                if info.si_signo == Signal::SIGSTOP as i32
                    || info.si_signo == Signal::SIGKILL as i32
                    || !mask.test(info.si_signo as usize)
                {
                    let tmp = *info;
                    if !is_poll {
                        iter.remove_current();
                    }
                    return Some(tmp);
                }
                iter.move_next();
            }
        }

        None
    }

    /// Send a signal to this thread.
    pub fn send_async_sig(&self, info: siginfo_t) {
        debug_assert!(info.si_signo < 1024);
        self.sigqueue.lock().push_back(info);
        // Causes interruptable locks to return Err(EINTR).
        self.flags.fetch_and(!tflags::BLOCKED, Ordering::Relaxed);
    }

    /// Unblock this thread early.
    pub fn unblock(&self) {
        self.flags.fetch_and(!tflags::BLOCKED, Ordering::Relaxed);
    }
}

/// Number of currently running schedulers.
pub(super) static RUNNING_SCHED_COUNT: AtomicU32 = AtomicU32::new(0);
/// Global list of threads to be reaped.
static ZOMBIES: Spinlock<ArcInvasiveList<Thread>> = Spinlock::new(ArcInvasiveList::new());
/// The thread reaper thread.
static REAPER: Spinlock<Option<Arc<Thread>>> = Spinlock::new(None);

/// Instance of a scheduler running on one CPU.
pub struct Scheduler {
    /// Idle thread for this scheduler.
    idle: Option<Arc<Thread>>,
    /// Runnable thread queue.
    queue: Spinlock<ActiveSchedAlgorithm>,
    /// Implements RCU semantics.
    rcu: RcuCtx,
    /// How many ticks until the current thread is preempted.
    /// If set to 0, the thread will not be preempted.
    preempt_ticks: u32,
    /// Last microsecond timestamp at which time usage was accounted.
    last_account_us: i64,
    /// Bit-set used as a ringbuffer to measure load average.
    active_set: BitSet<{ config::LOAD_MEASURE_WINDOW as usize }>,
    /// Which bit within `active_ticks` is written next.
    active_next: u32,
    /// Atomically-updated sum of active ticks within the last LOAD_MEASURE_WINDOW ticks.
    load_average: AtomicI32,
}

impl Scheduler {
    pub fn new() -> EResult<Self> {
        let mut queue = ActiveSchedAlgorithm::new();

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
        let _ = queue.add_thread_front(up_reporter);

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
                let _ = queue.add_thread(tcb);
            }
        }

        Ok(Self {
            idle: Some(idle),
            queue: Spinlock::new(queue),
            rcu: RcuCtx::new(),
            preempt_ticks: 0,
            last_account_us: time_us(),
            active_set: BitSet::EMPTY,
            active_next: 0,
            load_average: AtomicI32::new(0),
        })
    }

    /// Thread reaper function.
    fn reaper_func() -> ! {
        let thread_self = unsafe { &*Thread::current() };
        loop {
            thread_yield();

            // Threads are transferred to this list and dropped at the end of the loop.
            let mut threads = ArcInvasiveList::new();

            let _noirq = IrqGuard::new();

            unsafe { thread_self.runtime().timeout = timestamp_us_t::MAX };
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
        }
    }

    /// Scheduler idle function.
    fn idle_func() -> ! {
        let cur_cpu = smp::cur_cpu();
        let cur_sched = unsafe { &mut *Scheduler::get() };
        loop {
            let mut highest_load = 0;
            let mut busiest_cpu = None;

            for cpu in 0..smp::cpu_index_end() {
                let sched = if cpu != cur_cpu
                    && let Some(sched) = smp::get_sched_for(cpu)
                {
                    sched
                } else {
                    continue;
                };

                let load = sched.load_average.load(Ordering::Relaxed);
                if busiest_cpu.is_some() && highest_load >= load {
                    continue;
                }

                busiest_cpu = Some(cpu);
                highest_load = load;
            }

            let _ = try {
                let _noirq = IrqGuard::new();
                let sched = smp::get_sched_for(busiest_cpu?)?;
                let mut queue = sched.queue.lock();
                let thread = queue.choose_thread()?;
                // queue.remove_thread(thread);
                cur_sched.queue.lock().add_thread(thread);
            };

            thread_yield();
            pause_hint();
        }
    }

    /// Start this scheduler on the local CPU.
    pub unsafe fn exec(&mut self) -> ! {
        RUNNING_SCHED_COUNT.fetch_add(1, Ordering::Relaxed);
        self.rcu.post_start_callback();
        cpu::timer::start_tick_timer();
        self.sched_yield();
        unreachable!();
    }

    /// Yield the current thread's execution.
    fn sched_yield(&mut self) {
        debug_assert!(!cpu::irq::is_enabled());
        self.rcu.sched_callback();
        unsafe {
            let cpulocal = &mut *CpuLocal::get();
            let mut old = None;
            core::mem::swap(&mut old, &mut cpulocal.thread);

            // Put the running thread back in the queue.
            let mut queue = self.queue.lock();
            let mut dummy = null_mut();
            let old_stack_out: *mut *mut ();
            if let Some(old) = old {
                old.flags.fetch_and(!tflags::RUNNING, Ordering::Relaxed);
                old_stack_out = &raw mut old.runtime.as_mut_unchecked().as_mut().unwrap().stack_ptr;
                if self.idle.is_none() {
                    // Or in the idle slot, if the idle thread was running.
                    self.idle = Some(old);
                } else {
                    queue.return_thread(old);
                }
            } else {
                old_stack_out = &raw mut dummy;
            }

            let next = queue.choose_thread().unwrap_or_else(|| {
                let mut next = None;
                core::mem::swap(&mut next, &mut self.idle);
                next.unwrap()
            });
            next.flags.fetch_or(tflags::RUNNING, Ordering::Relaxed);

            self.preempt_ticks = (config::TICKS_PER_SEC as u32).div_ceil(100);

            // Switch to next page table.
            let runtime = next.runtime();
            if !runtime.memmap.is_null() {
                (&*runtime.memmap).enable();
            } else {
                vmm::kernel_mm().enable();
            }

            // Context switch into new thread.
            cpulocal.arch.set_irq_stack(runtime.irq_stack);
            let new_stack = &raw const runtime.stack_ptr;
            cpulocal.thread = Some(next);

            // The queue cannot be transmitted through this so we will manually unlock it afterward.
            core::mem::forget(queue);
            let prev = context_switch(self, new_stack, old_stack_out);
            drop(RawSpinlockGuard::from_raw(&(&*prev).queue.inner()))
        }
    }

    /// Account current thread's time usage since it was last accounted.
    /// If `as_user_time` is true, it is counted as userspace time.
    /// Otherwise, it is counted as kernel time.
    pub fn account_time(&mut self, as_user_time: bool) {
        let now = time_us();
        let delta = now - self.last_account_us;
        self.last_account_us = now;
        let thread = unsafe { &*Thread::current() };
        if as_user_time {
            thread.utime.fetch_add(delta, Ordering::Relaxed);
        } else {
            thread.ktime.fetch_add(delta, Ordering::Relaxed);
        }
    }

    /// Called every time a timer tick interrupt happens.
    /// Accounts thread time usage and manages preemption.
    /// If `as_user_time` is true, it is counted as userspace time.
    /// Otherwise, it is counted as kernel time.
    pub fn tick_interrupt(&mut self, is_user_time: bool) {
        self.account_time(is_user_time);
        cpu::timer::start_tick_timer();
        panic::check_for_panic();

        // Measure load average.
        let was_active = self.active_set.test(self.active_next as usize);
        let now_active = self.idle.is_none();
        let delta = now_active as i32 - was_active as i32;
        self.load_average.fetch_add(delta, Ordering::Relaxed);
        self.active_next = (self.active_next + 1) % config::LOAD_MEASURE_WINDOW as u32;

        if self.preempt_ticks > 0 {
            self.preempt_ticks -= 1;
            if self.preempt_ticks == 0 {
                self.sched_yield();
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
            sched.sched_yield();
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
    use core::ffi::c_void;

    use crate::bindings::{
        error::Errno,
        raw::{errno_t, timestamp_us_t},
    };

    use super::Thread;

    #[unsafe(no_mangle)]
    extern "C" fn thread_sleep(amount: timestamp_us_t) -> errno_t {
        Errno::extract(super::thread_sleep(amount))
    }

    #[unsafe(no_mangle)]
    extern "C" fn thread_current() -> *mut c_void {
        Thread::current() as *mut c_void
    }
}
