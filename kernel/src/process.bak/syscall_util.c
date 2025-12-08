
// SPDX-License-Identifier: MIT

#include "syscall_util.h"

#include "process/internal.h"
#include "process/sighandler.h"
#include "process/types.h"


// Assert that a condition is true, or raise SIGSEGV and don't return.
void sigsys_assert(bool condition) {
    if (!condition) {
        proc_sigsys_handler();
    }
}

// Assert that a condition is true, or raise SIGSEGV and don't return.
void sigsegv_assert(bool condition, size_t vaddr) {
    if (!condition) {
        proc_sigsegv_handler(vaddr);
    }
}

// Checks whether the process has permission for a range of memory.
bool sysutil_memperm(void const *ptr, size_t len, uint32_t flags) {
    return (proc_map_contains_raw(proc_current(), (size_t)ptr, len) & flags) == flags;
}

// If the process does not have access, raise SIGSEGV and don't return.
void sysutil_memassert(void const *ptr, size_t len, uint32_t flags) {
    if (!sysutil_memperm(ptr, len, flags)) {
        proc_sigsegv_handler((size_t)ptr);
    }
}
