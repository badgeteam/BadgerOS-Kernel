
// SPDX-License-Identifier: MIT

#pragma once

#include "attributes.h"
#include "errno.h"
#include "time.h"

#include <stdatomic.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// Processs struct.
typedef struct process      process_t;
// Globally unique thread ID; 1 to INT_MAX.
typedef int                 tid_t;
// Thread struct.
typedef struct sched_thread sched_thread_t;
// Kernel thread entrypoint.
typedef int (*sched_entry_t)(void *arg);
// CPU-local scheduler data.
typedef struct sched_cpulocal sched_cpulocal_t;

// Number of CPUs with running schedulers.
extern atomic_int running_sched_count;

// Time usage information.
typedef struct {
    // Time spent running in user-space.
    timestamp_us_t user_time;
    // Time spent running in kernel-space.
    timestamp_us_t kernel_time;
    // Total time usage since last load measurement.
    timestamp_us_t cycle_time;
    // Current number of CPUs used in 0.01% units.
    atomic_int     cpu_usage;
} timeusage_t;

// will be scheduled with smaller time slices than normal
#define SCHED_PRIO_LOW    0
// default value
#define SCHED_PRIO_NORMAL 10
// will be scheduled with bigger time slices than normal
#define SCHED_PRIO_HIGH   20



// Global scheduler initialization.
void sched_init();
// Power on and start scheduler on secondary CPUs.
void sched_start_altcpus();
// Power on and start scheduler on another CPU.
bool sched_start_on(int cpu);
// Prepare a new scheduler context for this or another CPU.
void sched_init_cpu(int cpu);
// Start executing the scheduler on this CPU after `sched_init_cpu` was called for this CPU.
void sched_exec() NORETURN;
// Exit the scheduler and subsequenty shut down the CPU.
void sched_exit(int cpu);


// Returns the current thread ID.
tid_t           sched_current_tid();
// Returns the current thread struct.
sched_thread_t *sched_current_thread();
// Returns the associated thread struct.
sched_thread_t *sched_get_thread(tid_t thread);

// Create a new suspended userland thread.
// If `kernel_stack_bottom` is NULL, the scheduler will allocate a stack.
tid_t   thread_new_user(char const *name, process_t *process, size_t user_entrypoint, size_t user_arg, int priority);
// Create a new user thread with a cloned user context.
tid_t   thread_fork(tid_t thread, process_t *new_process);
// Create new suspended kernel thread.
// If `stack_bottom` is NULL, the scheduler will allocate a stack.
tid_t   thread_new_kernel(char const *name, sched_entry_t entry_point, void *arg, int priority);
// Do not wait for thread to be joined; clean up immediately.
errno_t thread_detach(tid_t thread);

// Explicitly yield to the scheduler; the scheduler may run other threads without waiting for preemption.
// Use this function to reduce the CPU time used by a thread.
void thread_yield(void);
// Sleep for an amount of microseconds.
void thread_sleep(timestamp_us_t delay);

// Pauses execution of a thread.
// If `suspend_kernel` is false, the thread won't be suspended until it enters user mode.
errno_t      thread_suspend(tid_t thread, bool suspend_kernel);
// Resumes a previously suspended thread or starts it.
errno_t      thread_resume(tid_t thread);
// Resumes a previously suspended thread or starts it.
// Immediately schedules the thread instead of putting it in the queue first.
errno_t      thread_resume_now(tid_t thread);
// Resumes a previously suspended thread or starts it from an ISR.
errno_t      thread_resume_from_isr(tid_t thread);
// Resumes a previously suspended thread or starts it from an ISR.
// Immediately schedules the thread instead of putting it in the queue first.
errno_t      thread_resume_now_from_isr(tid_t thread);
// Returns whether a thread is running; it is neither suspended nor has it exited.
errno_bool_t thread_is_running(tid_t thread);

// Exits the current thread.
// If the thread is detached, resources will be cleaned up.
void thread_exit(int code) NORETURN;
// Wait for another thread to exit.
int  thread_join(tid_t thread);

// Dump scheduler state for debug purposes.
void sched_debug_print();
