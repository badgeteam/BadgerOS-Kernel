
// SPDX-License-Identifier: MIT

#pragma once

#include "attributes.h"

#include <stddef.h>

// Kernel side of the signal handler.
// Called in the kernel side of a used thread when a signal might be queued.
void proc_signal_handler() NORETURN;
// Raises a segmentation fault to the current thread.
// Called in the kernel side of a used thread when hardware detects a segmentation fault.
void proc_sigsegv_handler(size_t vaddr) NORETURN;
// Raises an illegal instruction fault to the current thread.
// Called in the kernel side of a used thread when hardware detects an illegal instruction fault.
void proc_sigill_handler() NORETURN;
// Raises a trace/breakpoint trap to the current thread.
// Called in the kernel side of a used thread when hardware detects a trace/breakpoint trap.
void proc_sigtrap_handler() NORETURN;
// Raises an invalid system call to the current thread.
// Called in the kernel side of a used thread when a system call does not exist.
void proc_sigsys_handler() NORETURN;
