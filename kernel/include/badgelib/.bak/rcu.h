
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

// CPU-local RCU state.
typedef struct {
    int generation;
#ifndef NDEBUG
    int depth;
#endif
} rcu_cpulocal_t;

#include "assertions.h"
#include "cpulocal.h"
#include "interrupt.h"
#include "isr_ctx.h"

// Scheduler function callback; checks for the advance of an RCU generation.
// WARNING: Must be called one final time *after* running_sched_count is decreased on scheduler exit!
// WARNING: Do not call from any other code than the actual scheduling function!
void rcu_sched_func_callback(rcu_cpulocal_t *rcu_cpulocal);
// Scheduler start callback; spins until an RCU generation passes.
// WARNING: Must be called *after* running_sched_count is increased to add this scheduler!
// WARNING: Do not call from any other code than the scheduler start function, ONCE!
void rcu_sched_init_callback(rcu_cpulocal_t *rcu_cpulocal);

#ifndef NDEBUG
// Enter RCU critical section.
#define rcu_crit_enter()                                                                                               \
    do {                                                                                                               \
        isr_ctx_get()->cpulocal->rcu.depth++;                                                                          \
        assert_always(!irq_is_enabled());                                                                              \
    } while (0)
// Exit RCU critical section.
#define rcu_crit_exit()   assert_always(isr_ctx_get()->cpulocal->rcu.depth-- > 0)
// Debug-assert that code is in an RCU critical section.
#define rcu_crit_assert() assert_dev_drop(isr_ctx_get()->cpulocal->rcu.depth > 0)
#else
// Enter RCU critical section.
#define rcu_crit_enter() assert_dev_drop(!irq_is_enabled())
// Exit RCU critical section.
#define rcu_crit_exit()  (void)0
// Debug-assert that code is in an RCU critical section.
#define rcu_crit_assert()
#endif

// Synchronize RCU for reclamation.
void rcu_sync();
// Read an RCU pointer; takes a pointer to the pointer that is RCU.
#define rcu_read_ptr(rcu_ptr_ptr)                                                                                      \
    do {                                                                                                               \
        rcu_crit_assert();                                                                                             \
        atomic_load_explicit(rcu_ptr_ptr, memory_order_acquire);                                                       \
    } while (0)
// Write an RCU pointer; takes a pointer to the pointer that is RCU.
#define rcu_write_ptr(rcu_ptr_ptr, new_ptr) atomic_store_explicit(rcu_ptr_ptr, new_ptr, memory_order_acq_rel)
// Write/exchange an RCU pointer; takes a pointer to the pointer that is RCU.
#define rcu_xchg_ptr(rcu_ptr_ptr, new_ptr)  atomic_exchange_explicit(rcu_ptr_ptr, new_ptr, memory_order_acq_rel)
