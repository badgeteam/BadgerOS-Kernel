// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include <stdatomic.h>


#define rcu_crit_enter()  ((void)0)
#define rcu_crit_exit()   ((void)0)
#define rcu_crit_assert() ((void)0)


// Synchronize RCU for reclamation.
void rcu_sync();
// Read an RCU pointer; takes a pointer to the pointer that is RCU.
#define rcu_read_ptr(rcu_ptr_ptr)           atomic_load_explicit(rcu_ptr_ptr, memory_order_acquire)
// Write an RCU pointer; takes a pointer to the pointer that is RCU.
#define rcu_write_ptr(rcu_ptr_ptr, new_ptr) atomic_store_explicit(rcu_ptr_ptr, new_ptr, memory_order_acq_rel)
// Write/exchange an RCU pointer; takes a pointer to the pointer that is RCU.
#define rcu_xchg_ptr(rcu_ptr_ptr, new_ptr)  atomic_exchange_explicit(rcu_ptr_ptr, new_ptr, memory_order_acq_rel)
