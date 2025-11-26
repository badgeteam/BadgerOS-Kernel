
// SPDX-License-Identifier: MIT

#pragma once

#include "cpu/riscv.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// Note: These flags are copied from the Rust code, do not change them!

// Map memory as executable.
#define VMM_FLAG_R 0b000000000010
// Map memory as writeable (reads must also be allowed).
#define VMM_FLAG_W 0b000000000100
// Map memory as executable.
#define VMM_FLAG_X 0b000000001000
// Map memory as user-accessible.
#define VMM_FLAG_U 0b000000010000
// Map memory as global (exists in all page ASIDs).
#define VMM_FLAG_G 0b000000100000
// Page was accessed since this flag was last cleared.
#define VMM_FLAG_A 0b000001000000
// Page was written since this flag was last cleared.
#define VMM_FLAG_D 0b000010000000

// Mark page as copy-on-write (W must be disabled).
#define VMM_FLAG_COW 0b000100000000

// Map memory as I/O (uncached, no write coalescing).
#define VMM_FLAG_IO 0b010000000000
// Map memory as uncached write coalescing.
#define VMM_FLAG_NC 0b100000000000

// Mark page as copy-on-write (W must be disabled).
#define VMM_FLAG_COW  0b000100000000
// Mark page as shared (will not be turned int CoW on fork).
#define VMM_FLAG_SHM  0b001000000000
// Mark page as memory-mapped I/O (anything except normal RAM; informational in case hardare doesn't support this flag).
#define VMM_FLAG_MMIO 0b001100000000
// What kind of memory is mapped at this page.
#define VMM_FLAG_MODE 0b001100000000



// Enable supervisor access to user memory.
static inline void mmu_enable_sum() {
    asm("csrs sstatus, %0" ::"r"((1 << RISCV_STATUS_SUM_BIT) | (1 << RISCV_STATUS_MXR_BIT)));
}
// Disable supervisor access to user memory.
static inline void mmu_disable_sum() {
    asm("csrc sstatus, %0" ::"r"((1 << RISCV_STATUS_SUM_BIT) | (1 << RISCV_STATUS_MXR_BIT)));
}



// Notify the MMU of global mapping changes.
static inline void mmu_vmem_fence() {
    asm volatile("fence rw,rw" ::: "memory");
    asm volatile("sfence.vma" ::: "memory");
}
