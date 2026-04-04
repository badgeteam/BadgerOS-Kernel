
// SPDX-License-Identifier: MIT

#pragma once

#include "cpu/riscv.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// Note: These flags are copied from the Rust code, do not change them!

// Map memory as executable.
#define MMU_FLAG_R 0b000000000010
// Map memory as writeable (reads must also be allowed).
#define MMU_FLAG_W 0b000000000100
// Map memory as executable.
#define MMU_FLAG_X 0b000000001000
// Map memory as user-accessible.
#define MMU_FLAG_U 0b000000010000
// Map memory as global (exists in all page ASIDs).
#define MMU_FLAG_G 0b000000100000
// Page was accessed since this flag was last cleared.
#define MMU_FLAG_A 0b000001000000
// Page was written since this flag was last cleared.
#define MMU_FLAG_D 0b000010000000

// Mark page as copy-on-write (W must be disabled).
#define MMU_FLAG_COW 0b000100000000

// Map memory as I/O (uncached, no write coalescing).
#define MMU_FLAG_IO 0b010000000000
// Map memory as uncached write coalescing.
#define MMU_FLAG_NC 0b100000000000



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
