// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

use crate::{bindings::raw::smp_cur_cpu, scheduler::RUNNING_SCHED_COUNT};

/// Global RCU generation.
static RCU_GENERATION: AtomicU32 = AtomicU32::new(0);
/// CPUs that still have to pass the RCU generation.
static RCU_OUTSTANDING: AtomicU32 = AtomicU32::new(1);

/// Per-scheduler RCU context.
pub(in crate::scheduler) struct RcuCtx {
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
        // TODO: This startup detection will fail if for any reason CPU0
        // goes to sleep and wakes up while some other CPU is still awake.
        if unsafe { smp_cur_cpu() } != 0 {
            let old_gen = RCU_GENERATION.load(Ordering::Relaxed);
            while RCU_GENERATION.load(Ordering::Relaxed) == old_gen {}
        }

        self.generation = RCU_GENERATION.load(Ordering::Relaxed);
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
        RCU_OUTSTANDING.store(
            RUNNING_SCHED_COUNT.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
        RCU_GENERATION.fetch_add(1, Ordering::Relaxed);
    }
}
