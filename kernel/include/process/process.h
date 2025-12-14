
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "mem/vmm.h"

#include <stdatomic.h>
#include <stdint.h>

#ifdef __clang__
#define __user __attribute__((noderef, address_space(1)))
#else
#define __user
#endif

#define PROC_FLAG_STOPPING (1u << 0)

// Unique process identifier.
typedef int64_t pid_t;

// The process descriptor structure.
typedef struct process process_t;

// Needed by C because the process struct is not representable in C.
vmm_ctx_t   *proc_memmap(process_t *proc);
// Needed by C because the process struct is not representable in C.
atomic_uint *proc_flags(process_t *proc);
// Needed by C because the process struct is not representable in C.
pid_t        proc_pid(process_t *proc);
// Start the init process.
void         proc_start_init();

// Called when SIGSEGV is raised by a trap.
void proc_pagefault_handler();
// Called when SIGILL is raised by a trap.
void proc_sigill_handler();
// Called when SIGTRAP is raised by a trap.
void proc_sigtrap_handler();
