
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

// This file is for temporary backwards compatibility with the old static-buddy.c API for page allocations.

#include "badge_strings.h"
#include "log.h"
#include "mem/pmm.h"
#include "mem/vmm.h"
#include "panic.h"
#include "static-buddy.h"

void *buddy_allocate(size_t size, enum block_type type, uint32_t flags) {
    page_usage_t usage;
    switch (type) {
        default: panic_abort();
        case BLOCK_TYPE_USER: usage = PAGE_USAGE_USER_ANON; break;
        case BLOCK_TYPE_PAGE: usage = PAGE_USAGE_KERNEL_ANON; break;
        case BLOCK_TYPE_SLAB: usage = PAGE_USAGE_KERNEL_SLAB; break;
    }
    uint32_t order = 0;
    while (((size_t)CONFIG_PAGE_SIZE << order) < size) {
        order++;
    }
    errno_size_t page = pmm_page_alloc(order, usage);
    if (page < 0) {
        logkf(LOG_WARN, "Out of memory (allocating block of order %{u32;d})", order);
        return NULL;
    }
    return (void *)(page + vmm_hhdm_offset);
}

void *buddy_reallocate(void *ptr, size_t size) {
    if (!ptr) {
        buddy_deallocate(ptr);
        return NULL;
    }

    size_t old_size = buddy_get_size(ptr);

    if (old_size >= size) {
        return ptr;
    }

    void *newmem = buddy_allocate(size, buddy_get_type(ptr), 0);
    if (!newmem) {
        return NULL;
    }
    size_t copy_size = old_size < size ? old_size : size;
    mem_copy(newmem, ptr, copy_size);

    buddy_deallocate(ptr);

    return newmem;
}

void buddy_deallocate(void *ptr) {
    pmm_page_t *page_meta = pmm_page_struct((size_t)ptr - vmm_hhdm_offset);
    pmm_page_free((size_t)ptr - vmm_hhdm_offset, page_meta->order);
}

enum block_type buddy_get_type(void *ptr) {
    pmm_page_t *page_meta = pmm_page_struct((size_t)ptr - vmm_hhdm_offset);
    switch ((page_usage_t)page_meta->usage) {
        case PAGE_USAGE_FREE: return BLOCK_TYPE_FREE;
        case PAGE_USAGE_PAGE_TABLE:
        case PAGE_USAGE_KERNEL_ANON:
        case PAGE_USAGE_UNUSABLE:
        case PAGE_USAGE_CACHE:
        default: return BLOCK_TYPE_PAGE;
        case PAGE_USAGE_KERNEL_SLAB: return BLOCK_TYPE_SLAB;
        case PAGE_USAGE_MMAP:
        case PAGE_USAGE_USER_ANON: return BLOCK_TYPE_USER;
    }
}

size_t buddy_get_size(void *ptr) {
    pmm_page_t *page_meta = pmm_page_struct((size_t)ptr - vmm_hhdm_offset);
    return (size_t)CONFIG_PAGE_SIZE << page_meta->order;
}
