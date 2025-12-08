
// SPDX-License-Identifier: MIT

#pragma once

#include "errno.h"
#include "scheduler/scheduler.h"
#include "signal.h"



// Process is running or waiting for syscalls.
#define PROC_RUNNING  0x00000001
// Process is waiting for threads to exit.
#define PROC_EXITING  0x00000002
// Process has fully exited.
#define PROC_EXITED   0x00000004
// Process has signals pending.
#define PROC_SIGPEND  0x00000008
// Process is pre-start.
#define PROC_PRESTART 0x00000010
// Process has a state change not acknowledged.
#define PROC_STATECHG 0x00000020



// A process and all of its resources.
typedef struct process_t process_t;
// Globally unique process ID.
typedef int              pid_t;

// Send a signal to all running processes in the system except the init process.
void proc_signal_all(int signal);
// Whether any non-init processes are currently running.
bool proc_has_noninit();

// Create a new, empty process.
pid_t     proc_create(pid_t parent, char const *binary, int argc, char const *const *argv);
// Delete a process only if it hasn't been started yet.
errno_t   proc_delete_prestart(pid_t pid);
// Delete a process and release any resources it had.
errno_t   proc_delete(pid_t pid);
// Get the process' flags.
errno64_t proc_getflags(pid_t pid);
// Get the PID of the current process, if any.
pid_t     proc_current_pid() __attribute__((const));
// Load an executable and start a prepared process.
errno_t   proc_start(pid_t pid);

// Allocate more memory to a process.
// Returns actual virtual address on success, 0 on failure.
errno_size_t proc_map(pid_t pid, size_t vaddr_req, size_t min_size, size_t min_align, int flags);
// Release memory allocated to a process.
// The given span should not fall outside an area mapped with `proc_map_raw`.
errno_t      proc_unmap(pid_t pid, size_t vaddr, size_t len);
// Whether the process owns this range of memory.
// Returns the lowest common denominator of the access bits bitwise or 8.
// Returns -errno on error.
errno_t      proc_map_contains(pid_t pid, size_t base, size_t size);

// Raise a signal to a process, which may be the current process.
errno_t proc_raise_signal(pid_t pid, int signum);

// Check whether a process is a parent to another.
errno_bool_t proc_is_parent(pid_t parent, pid_t child);
