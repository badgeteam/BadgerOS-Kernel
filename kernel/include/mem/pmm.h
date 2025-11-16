
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "errno.h"

#include <stdatomic.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// Note: These definitions are copied from the Rust code, do not change them!

// Kinds of usage for pages of memory.
typedef enum {
    // Unused page.
    PAGE_USAGE_FREE = 0,
    // Part of a page table.
    PAGE_USAGE_PAGE_TABLE,
    // Contains cached data.
    PAGE_USAGE_CACHE,
    // Part of a mmap'ed file.
    PAGE_USAGE_MMAP,
    // Anonymous user memory.
    PAGE_USAGE_USER_ANON,
    // Anonymous kernel memory.
    PAGE_USAGE_KERNEL_ANON,
    // Kernel slabs memory (may be removed in the future).
    PAGE_USAGE_KERNEL_SLAB,
    // The actual kernel executable itself, be that code or data.
    PAGE_USAGE_KERNEL_SEGMENT,
    // Dummy entry for unusable page.
    PAGE_USAGE_UNUSABLE,
} page_usage_t;

// Page struct is locked for modifications.
#define PGFLAGS_LOCKED     0x00000001u
// Page may be swapped to disk.
#define PGFLAGS_SWAPPABLE  0x00000002u
// Bitmask: buddy alloc page order.
#define PGFLAGS_ORDER_MASK 0x00fc0000u
// Bit exponent: buddy alloc page order.
#define PGFLAGS_ORDER_EXP  18u
// Bitmask: Page usage.
#define PGFLAGS_USAGE_MASK 0xff000000u
// Bit exponent: Page usage.
#define PGFLAGS_USAGE_EXP  24u

// Physical memory page metadata.
typedef struct {
    // Page refcount, typically 1 for kernel pages.
    atomic_uint refcount;
    // Page flags and usage kind.
    atomic_uint flags;
    // TODO: Pointer to structure that exposes where it's mapped in user virtual memory.
    // Kernel virtual mappings need not be tracked because they are not swappable.
} pmm_page_t;

typedef size_t       ppn_t;
typedef errno_size_t errno_ppn_t;

// Allocate `1 << order` pages of physical memory.
// The initial refcount will be 1.
errno_ppn_t pmm_page_alloc(uint8_t order, page_usage_t usage);
// Decrease the refcount of a page of physical memory.
// Will mark the page as free if the refcount hits 0.
void        pmm_page_free(ppn_t block, uint8_t order);
// Get the `pmm_page_t` struct for some physical page number.
pmm_page_t *pmm_page_struct(ppn_t page);
// Get the `pmm_page_t` struct for the start of the block that some physical page number lies in.
pmm_page_t *pmm_page_struct_base(ppn_t page);
// Mark a range of blocks as free.
void        pmm_mark_free(ppn_t pages_start, ppn_t pages_end);
// Initialize the physical memory allocator.
// It is assumed that the boot protocol implementation hereafter marks the kernel executable with
// `PAGE_USAGE_KERNEL_SEGMENT`.
void        pmm_init(ppn_t total_start, ppn_t total_end, ppn_t early_start, ppn_t early_end);
