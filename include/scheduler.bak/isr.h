
// SPDX-License-Identifier: MIT

#pragma once

#include "scheduler/scheduler.h"



// Remove the current thread from the runqueue from this CPU.
// Interrupts must be disabled.
sched_thread_t *thread_dequeue_self();

// Requests the scheduler to prepare a switch from inside an interrupt routine.
void sched_request_switch_from_isr();

// Get a new blocking ticket; to block the thread, run `thread_yield()`.
uint64_t thread_block();

// Unblock a thread.
bool thread_unblock(tid_t thread, uint64_t ticket);
