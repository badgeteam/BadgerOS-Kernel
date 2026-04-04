
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "badgeros-abi/pid_t.h"
#include "mem/vmm.h"

#include <stdatomic.h>
#include <stdint.h>

#ifdef __clang__
#define __user __attribute__((noderef, address_space(1)))
#else
#define __user
#endif

#define PROC_FLAG_STOPPING (1u << 0)

// The process descriptor structure.
typedef struct process process_t;
