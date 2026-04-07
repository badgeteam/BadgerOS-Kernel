
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "badgeros-abi/errno.h"

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

// Physical memory page metadata.
typedef struct {
    // Page refcount, may be used for arbitrary purposes by the owner.
    // In virtual memory objects, this counts the number of times a page is mapped in a pmap.
    atomic_uint refcount;
    // Order of the buddy block this page belongs to.
    uint8_t     order;
    // Current page usage.
    uint8_t     usage;
    // TODO: Pointer to structure that exposes where it's mapped in user virtual memory.
    // Kernel virtual mappings need not be tracked because they are not swappable.
} pmm_page_t;

typedef size_t paddr_t;

// Allocate `1 << order` pages of physical memory.
// The initial refcount will be 1.
errno_size_t pmm_page_alloc(uint8_t order, page_usage_t usage);
// Decrease the refcount of a page of physical memory.
// Will mark the page as free if the refcount hits 0.
void         pmm_page_free(paddr_t block, uint8_t order);
// Get the `pmm_page_t` struct for some physical page number.
pmm_page_t  *pmm_page_struct(paddr_t page);
// Get the `pmm_page_t` struct for the start of the block that some physical page number lies in.
pmm_page_t  *pmm_page_struct_base(paddr_t page);
// Mark a range of blocks as free.
void         pmm_mark_free(paddr_t pages_start, paddr_t pages_end);
// Initialize the physical memory allocator.
// It is assumed that the boot protocol implementation hereafter marks the kernel executable with
// `PAGE_USAGE_KERNEL_SEGMENT`.
void         pmm_init(paddr_t total_start, paddr_t total_end, paddr_t early_start, paddr_t early_end);
