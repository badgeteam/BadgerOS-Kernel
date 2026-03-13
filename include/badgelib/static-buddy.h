// SPDX-License-Identifier: MIT

#pragma once
#ifndef BADGEROS_KERNEL
#define _GNU_SOURCE
#endif

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define PAGE_SIZE        4096
#define MAX_MEMORY_POOLS 16
#define MAX_SLAB_SIZE    256

#define ALIGN_UP(x, y)   (void *)(((size_t)(x) + (y - 1)) & ~(y - 1))
#define ALIGN_DOWN(x, y) (void *)((size_t)(x) & ~(y - 1))

#define ALIGN_PAGE_UP(x)   ALIGN_UP(x, PAGE_SIZE)
#define ALIGN_PAGE_DOWN(x) ALIGN_DOWN(x, PAGE_SIZE)

enum block_type { BLOCK_TYPE_FREE, BLOCK_TYPE_USER, BLOCK_TYPE_PAGE, BLOCK_TYPE_SLAB, BLOCK_TYPE_ERROR };
enum slab_type { SLAB_TYPE_SLAB };

void init_pool(void *mem_start, void *mem_end, uint32_t flags);
void init_kernel_slabs();
void print_allocator();

void  *slab_allocate(size_t size, enum slab_type type, uint32_t flags);
void   slab_deallocate(void *ptr);
size_t slab_get_size(void *ptr);

void           *buddy_allocate(size_t size, enum block_type type, uint32_t flags);
void           *buddy_reallocate(void *ptr, size_t size);
void            buddy_deallocate(void *ptr);
void            buddy_split_allocated(void *ptr);
enum block_type buddy_get_type(void *ptr);
size_t          buddy_get_size(void *ptr);

typedef struct buddy_block {
    uint8_t             pid;
    uint8_t             order;
    bool                in_list;
    bool                is_waste;
    enum block_type     type;
    struct buddy_block *next;
    struct buddy_block *prev;
} buddy_block_t;

typedef struct {
    uint32_t       flags;
    void          *start;
    void          *end;
    void          *pages_start;
    void          *pages_end;
    size_t         pages;
    size_t         free_pages;
    uint8_t        max_order;
    uint8_t        max_order_free;
    uint32_t       max_order_waste;
    buddy_block_t  waste_list;
    buddy_block_t *free_lists;
    buddy_block_t *blocks;
} memory_pool_t;

extern uint8_t       memory_pool_num;
extern memory_pool_t memory_pools[MAX_MEMORY_POOLS];
