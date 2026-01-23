// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

use crate::kernel::sched::{RUNNING_SCHED_COUNT, thread_yield};

use super::spinlock::Spinlock;

/// Global RCU generation.
static RCU_GENERATION: AtomicU32 = AtomicU32::new(0);
/// CPUs that still have to pass the RCU generation.
static RCU_OUTSTANDING: AtomicU32 = AtomicU32::new(1);
/// Totan number of CPUs participating in RCU.
static RCU_PARTICIPATING: Spinlock<u32> = Spinlock::new(0);

/// Per-scheduler RCU context.
pub(in crate::kernel) struct RcuCtx {
    /// What RCU generation this scheduler is on.
    generation: u32,
}

impl RcuCtx {
    pub fn new() -> Self {
        Self {
            generation: RCU_GENERATION.load(Ordering::Relaxed),
        }
    }

    pub fn post_start_callback(&mut self) {
        // This guard will stop the advancing CPU at the point between where it determines
        // that it should advance the global generation and where it stores to RCU_OUTSTANDING and RCU_GENERATION.
        let mut guard = RCU_PARTICIPATING.lock();

        if *guard == 0 {
            // First CPU to start, no others to wait on.
            RCU_OUTSTANDING.store(1, Ordering::Relaxed);
            RCU_GENERATION.store(0, Ordering::Relaxed);
            self.generation = 0;
            *guard = 1;
            return;
        }

        // Then wait for an RCU generation to elapse to synchronize
        while RCU_OUTSTANDING.load(Ordering::Relaxed) > 0 {}

        // Now this CPU is made a participant.
        self.generation = RCU_GENERATION.load(Ordering::Relaxed) + 1;
        *guard += 1;
    }

    pub fn post_stop_callback(&mut self) {
        self.sched_callback();
    }

    pub fn sched_callback(&mut self) {
        let generation = RCU_GENERATION.load(Ordering::Relaxed);
        debug_assert!(
            generation == self.generation || generation.wrapping_add(1) == self.generation
        );

        if generation != self.generation {
            return;
        }

        // Advance this CPU to the next RCU generation
        self.generation += 1;
        let outstanding = RCU_OUTSTANDING.fetch_sub(1, Ordering::Relaxed);
        debug_assert!(outstanding >= 1);

        if outstanding > 1 {
            return;
        }

        // All CPUs advanced, advance the global RCU generation.
        RCU_OUTSTANDING.store(RCU_PARTICIPATING.lock_shared().read(), Ordering::Relaxed);
        RCU_GENERATION.fetch_add(1, Ordering::Relaxed);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rcu_sync() {
    if RUNNING_SCHED_COUNT.load(Ordering::Relaxed) == 0 {
        return;
    }
    let generation = RCU_GENERATION.load(Ordering::Relaxed);
    while RCU_GENERATION.load(Ordering::Relaxed) == generation {
        thread_yield();
    }
}
