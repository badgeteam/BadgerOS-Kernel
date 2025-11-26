
// SPDX-License-Identifier: MIT

#pragma once

#include <stdint.h>



// PTE valid bit.
#define VMM_FLAG_P  (uint32_t)(0b000000001)
// Map memory as writeable (reads must also be allowed).
#define VMM_FLAG_W  (uint32_t)(0b000000010)
// Map memory as user-accessible.
#define VMM_FLAG_U  (uint32_t)(0b000000100)
// Map memory as I/O (uncached, no write coalescing).
#define VMM_FLAG_IO (uint32_t)(0b000001000)
// Map memory as uncached write coalescing.
#define VMM_FLAG_NC (uint32_t)(0b000010000)
// Page was accessed since this flag was last cleared.
#define VMM_FLAG_A  (uint32_t)(0b000100000)
// Page was written since this flag was last cleared.
#define VMM_FLAG_D  (uint32_t)(0b001000000)
// Map memory as global (exists in all page ASIDs).
#define VMM_FLAG_G  (uint32_t)(0b100000000)
// This is a hugepage leaf.
#define VMM_FLAG_PS (uint32_t)(0b010000000)

// Mark page as copy-on-write (W must be disabled).
#define VMM_FLAG_COW  (uint32_t)(0b001000000000)
// Mark page as shared (will not be turned into CoW on fork).
#define VMM_FLAG_SHM  (uint32_t)(0b010000000000)
// Mark page as memory-mapped I/O (anything except normal RAM; informational in case hardare doesn't support this flag).
#define VMM_FLAG_MMIO (uint32_t)(0b011000000000)
// What kind of memory is mapped at this page.
#define VMM_FLAG_MODE (uint32_t)(0b011000000000)

// Dummy readable flag.
#define VMM_FLAG_R (uint32_t)(1 << 16)
// Mark memory as executable (removes the XD flag).
#define VMM_FLAG_X (uint32_t)(1 << 17)



// Enable supervisor access to user memory.
static inline void mmu_enable_sum() {
}
// Disable supervisor access to user memory.
static inline void mmu_disable_sum() {
}



// Notify the MMU of global mapping changes.
static inline void mmu_vmem_fence() {
    asm volatile("mov rax, cr3; mov cr3, rax" ::: "rax");
}
