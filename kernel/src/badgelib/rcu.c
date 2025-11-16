
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "rcu.h"

#include "assertions.h"
#include "cpu/interrupt.h"
#include "cpu/isr.h"
#include "scheduler/scheduler.h"
#include "smp.h"

#include <stdatomic.h>

// Global RCU generation.
static atomic_int rcu_generation;
// CPUs that have to pass the RCU generation.
static atomic_int rcu_cpus_left = 1;

// Scheduler function callback; checks for the advance of an RCU generation.
// WARNING: Must be called one final time *after* running_sched_count is decreased on scheduler exit!
// WARNING: Do not call from any other code than the actual scheduling function!
void rcu_sched_func_callback(rcu_cpulocal_t *rcu_cpulocal) {
    int generation = atomic_load_explicit(&rcu_generation, memory_order_relaxed);
    assert_dev_drop(generation == rcu_cpulocal->generation || generation == rcu_cpulocal->generation - 1);

    if (generation == rcu_cpulocal->generation) {
        // Have CPU go to the next generation.
        rcu_cpulocal->generation++;
        int count = atomic_fetch_sub_explicit(&rcu_cpus_left, 1, memory_order_relaxed) - 1;
        assert_dev_drop(count >= 0);

        if (count == 0) {
            // All CPUs that are up have reached the
            atomic_store_explicit(
                &rcu_cpus_left,
                atomic_load_explicit(&running_sched_count, memory_order_relaxed),
                memory_order_relaxed
            );
            atomic_fetch_add_explicit(&rcu_generation, 1, memory_order_relaxed);
        }
    }
}

// Scheduler start callback; spins until an RCU generation passes.
// WARNING: Must be called *after* running_sched_count is increased to add this scheduler!
// WARNING: Do not call from any other code than the scheduler start function, ONCE!
void rcu_sched_init_callback(rcu_cpulocal_t *rcu_cpulocal) {
    assert_dev_drop(!irq_is_enabled());
    int cur_gen = atomic_load_explicit(&rcu_generation, memory_order_relaxed);

    // Only CPU0 does not wait for a generation to pass as that won't happen unit it itself starts the scheduler.
    if (smp_cur_cpu() != 0) {
        while (atomic_load_explicit(&rcu_generation, memory_order_relaxed) - cur_gen <= 0) {
            isr_pause();
        }
    }

    rcu_cpulocal->generation = atomic_load_explicit(&rcu_generation, memory_order_relaxed);
}

// Synchronize RCU for reclamation.
void rcu_sync() {
    if (running_sched_count == 0) {
        // Before the scheduler is up, an RCU sync is not needed and this implementation of it cannot work.
        return;
    }
    assert_dev_drop(irq_is_enabled());
    int cur_gen = atomic_load_explicit(&rcu_generation, memory_order_relaxed);
    while (atomic_load_explicit(&rcu_generation, memory_order_relaxed) - cur_gen <= 0) {
        thread_yield();
    }
}
