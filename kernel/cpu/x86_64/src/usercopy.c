
// SPDX-License-Identifier: MIT

#include "usercopy.h"

#include "assertions.h"
#include "badge_strings.h"
#include "cpu/mmu.h"
#include "interrupt.h"
#include "isr_ctx.h"
#include "mem/vmm.h"
#include "process/internal.h"

// TODO: Convert to page fault intercepting memcpy.
// TODO: Migrate into generic process API.



// Determine string length in memory a user owns.
// Returns -1 if the user doesn't have access to any byte in the string.
ptrdiff_t strlen_from_user_raw(process_t *process, size_t user_vaddr, ptrdiff_t max_len) {
    if (!max_len) {
        return 0;
    }
    ptrdiff_t len = 0;

    // Check first page permissions.
    if (!(proc_map_contains_raw(process, user_vaddr, 1) & VMM_FLAG_R)) {
        return -1;
    }

    // String length loop.
    vmm_ctx_t *old_mm = isr_ctx_get()->mem_ctx;
    vmm_ctxswitch(&process->memmap.mem_ctx);
    mmu_enable_sum();
    while (len < max_len && *(char const *)user_vaddr) {
        len++;
        user_vaddr++;
        if (user_vaddr % CONFIG_PAGE_SIZE == 0) {
            // Check further page permissions.
            mmu_disable_sum();
            if (!(proc_map_contains_raw(process, user_vaddr, 1) & VMM_FLAG_R)) {
                len = -1;
                break;
            }
            mmu_enable_sum();
        }
    }
    mmu_disable_sum();

    if (old_mm) {
        vmm_ctxswitch(old_mm);
    }
    return len;
}

// Copy bytes from user to kernel.
// Returns whether the user has access to all of these bytes.
// If the user doesn't have access, no copy is performed.
bool copy_from_user_raw(process_t *process, void *kernel_vaddr, size_t user_vaddr, size_t len) {
    if (!proc_map_contains_raw(process, user_vaddr, len)) {
        return false;
    }
    vmm_ctx_t *old_mm = isr_ctx_get()->mem_ctx;
    vmm_ctxswitch(&process->memmap.mem_ctx);
    mmu_enable_sum();
    mem_copy(kernel_vaddr, (void *)user_vaddr, len);
    mmu_disable_sum();
    if (old_mm) {
        vmm_ctxswitch(old_mm);
    }
    return true;
}

// Copy from kernel to user.
// Returns whether the user has access to all of these bytes.
// If the user doesn't have access, no copy is performed.
bool copy_to_user_raw(process_t *process, size_t user_vaddr, void const *kernel_vaddr0, size_t len) {
    if (!proc_map_contains_raw(process, user_vaddr, len)) {
        return false;
    }
    vmm_ctx_t *old_mm = isr_ctx_get()->mem_ctx;
    vmm_ctxswitch(&process->memmap.mem_ctx);
    mmu_enable_sum();
    mem_copy((void *)user_vaddr, kernel_vaddr0, len);
    mmu_disable_sum();
    if (old_mm) {
        vmm_ctxswitch(old_mm);
    }
    return true;
}
