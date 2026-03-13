
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __riscv
#if __riscv_xlen == 64
#define MMU_BITS_PER_LEVEL 9
#elif __riscv_xlen == 32
#define MMU_BITS_PER_LEVEL 10
#else
#error "Unsupported __riscv_xlen"
#endif
#elif defined(__x86_64__)
#define MMU_BITS_PER_LEVEL 9
#else
#error "Unsupported architecture"
#endif

// Note: Marked as const because these functions will only be called after VMM init is done.

// Determine whether an address is canonical.
bool mmu_is_canon_addr(uintptr_t addr) __attribute__((const));
// Determine whether an address is a canonical kernel address.
bool mmu_is_canon_kernel_addr(uintptr_t addr) __attribute__((const));
// Determine whether an address is a canonical user address.
bool mmu_is_canon_user_addr(uintptr_t addr) __attribute__((const));
// Determine whether a range is canonical.
bool mmu_is_canon_range(uintptr_t start, size_t len) __attribute__((const));
// Determine whether a range is a canonical kernel range.
bool mmu_is_canon_kernel_range(uintptr_t start, size_t len) __attribute__((const));
// Determine whether a range is a canonical user range.
bool mmu_is_canon_user_range(uintptr_t start, size_t len) __attribute__((const));

// Get the size of a "half" of the canonical ranges.
size_t    mmu_canon_half_size(void) __attribute__((const));
// Get the start of the higher half.
uintptr_t mmu_higher_half_vaddr(void) __attribute__((const));
// Get the number of paging levels active.
int       mmu_paging_levels(void) __attribute__((const));
