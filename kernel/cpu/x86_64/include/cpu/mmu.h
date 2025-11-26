
// SPDX-License-Identifier: MIT

#pragma once

#include "mem/vmm.h"



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
