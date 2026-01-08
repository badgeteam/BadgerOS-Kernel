
// SPDX-License-Identifier: MIT

#pragma once

#include "mem/vmm.h"
#include "syscall.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// Assert that a condition is true, or raise SIGSYS and don't return.
void sigsys_assert(bool condition);
// Assert that a condition is true, or raise SIGSEGV and don't return.
void sigsegv_assert(bool condition, size_t vaddr);
// Checks whether the process has permission for a range of memory.
bool sysutil_memperm(void const *ptr, size_t len, uint32_t flags);
// If the process does not have access, raise SIGSEGV and don't return.
void sysutil_memassert(void const *ptr, size_t len, uint32_t flags);
// Assert the process can read memory, or raise SIGSEGV and don't return.
#define sysutil_memassert_r(ptr, len)  sysutil_memassert(ptr, len, VMM_FLAG_R)
// Assert the process can read/write memory, or raise SIGSEGV and don't return.
#define sysutil_memassert_rw(ptr, len) sysutil_memassert(ptr, len, VMM_FLAG_RW)
// Assert the process can execute memory, or raise SIGSEGV and don't return.
#define sysutil_memassert_x(ptr, len)  sysutil_memassert(ptr, len, VMM_FLAG_X)
