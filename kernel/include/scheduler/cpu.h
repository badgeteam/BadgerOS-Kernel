
// SPDX-License-Identifier: MIT

#pragma once

#include "attributes.h"
#include "scheduler/scheduler.h"
#include "scheduler/types.h"
#include "signal.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>


// Requests the scheduler to prepare a switch from userland to kernel for a user thread.
// If `syscall` is true, copies argument registers to the kernel thread.
// The kernel thread will start at `entry_point`.
void sched_raise_from_isr(sched_thread_t *thread, bool syscall, void *entry_point);

// Requests the scheduler to prepare a switch from kernel to userland for a user thread.
// Resumes the userland thread where it left off.
void sched_lower_from_isr();

// Enters a signal handler in the current thread.
// Returns false if there isn't enough resources to do so.
bool sched_signal_enter(size_t handler_vaddr, size_t return_vaddr, siginfo_t siginfo);

// Exits a signal handler in the current thread.
// Returns false if the process cannot be resumed.
bool sched_signal_exit();

// Prepares a context to be invoked as a kernel thread.
void sched_prepare_kernel_entry(sched_thread_t *thread, void *entry_point, void *arg);

// Prepares a pair of contexts to be invoked as a userland thread.
// Kernel-side in these threads is always started by an ISR and the entry point is given at that time.
void sched_prepare_user_entry(sched_thread_t *thread, size_t entry_point, size_t arg);

// Run arch-specific task switch code before `isr_context_switch`.
// Called after the scheduler decides what thread to switch to.
void sched_arch_task_switch(sched_thread_t *next);
