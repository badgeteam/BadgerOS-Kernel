// SPDX-License-Identifier: MIT

/* Buddy Allocator for BadgerOS
 *
 * A Buddy allocator works by dividing memory up in power of 2 blocks. This means
 * that every block is exactly 2 blocks of a smaller size. This has the nice
 * property that it is very easy to figure out what block is "buddies" with another.
 *
 * Terminology:
 * - Order: The power of 2 size of a block
 * - Block: A group of pages of a particular order
 * - Buddy: Companion block of a block that together make up a block of one order
 *   higher
 * - Waste: Pages that fall within the power of 2 size the allocator manages, but
 *   don't correspond to usable memory
 *
 * By being able to find a buddy quickyly it because easy to collapse blocks into
 * larger blocks. For example lets say we have a whopping 16 total pages of memory:
 *
 * This means we have pages of the following orders:
 *
 * Order 4 : Blocks 0 - 15
 * Order 3 : Blocks 0 - 7, 8 - 15
 * Order 2 : Blocks 0 - 2, 3 - 7, 8 - 11, 12 - 15
 * Order 1 : Blocks 0 - 1, 2 - 3, 4 - 5, 6 - 7, 8 - 9, 10 - 11, 12 - 13, 14 - 15
 *
 * Each block has an index, finding the buddy of a block can thus be done quickly
 * if we know the index of the block and its order:
 *
 * buddy_index = index ^ (1lu << block->order)
 *
 * for instance, to find the buddy of block 8-11 order 2:
 *
 * buddy_index = 8 ^ (1 << 2) = 12
 *
 * If we then look at our list of blocks, we notice that blocks 8-11 + 12-15 indeed
 * form a block 8-15 of order 3.
 *
 * Because this is done with a xor operation, it is automatically reversible which
 * means that repeating the operation always yields the buddy of a block, regardles
 * of where you start. There's no need to explicitly keep track of buddies this way
 * nor are there any lookups.
 *
 * The allocator works by storing double linked lists of free blocks of each order.
 * When an allocation is made, we take the smallest block that can satisfy our
 * request, and if it is too big we split it down until we have a block of the size
 * we want. With every split a new free block of a lower order gets added to the
 * free list.
 *
 * The allocator starts by pushing all of the pages into a single block at the
 * highest order.
 *
 * Because this would mean we would only be able to manage memory that is an exact
 * power of 2 we introduce a feature called a waste page. A waste page is a page that
 * the buddy allocator tracks, but will never actually give out to callers. This means
 * that at initialization time we don't immediately start off with blocks of differing
 * sizes.
 *
 */

#include "static-buddy.h"

#include "badge_strings.h"
#include "bitops.h"
#include "debug.h"

#include <stdbool.h>
#include <stdint.h>

// Fine for our purposes here

#define MAX(a, b) ((a) > (b) ? (a) : (b))
#define MIN(a, b) ((a) < (b) ? (a) : (b))

uint8_t       memory_pool_num = 0;
memory_pool_t memory_pools[MAX_MEMORY_POOLS];

__attribute__((always_inline)) static inline memory_pool_t *ptr_to_pool(void *ptr) {
    for (int i = 0; i < memory_pool_num; ++i) {
        memory_pool_t *pool = &memory_pools[i];
        if (ptr >= pool->pages_start && ptr < pool->pages_end) {
            BADGEROS_MALLOC_MSG_DEBUG("ptr_to_pool(" FMT_P ") = " FMT_I, ptr, i);
            return pool;
        }
    }

    BADGEROS_MALLOC_MSG_DEBUG("ptr_to_pool() = NULL");
    return NULL;
}

__attribute__((always_inline)) static inline memory_pool_t *
    find_pool(uint8_t start_pool, uint8_t order, size_t alloc_size, uint32_t flags) {
    (void)flags;

    for (int i = start_pool; i < memory_pool_num; ++i) {
        memory_pool_t *pool = &memory_pools[i];
        if (pool->max_order_free >= order && pool->free_pages >= alloc_size) {
            BADGEROS_MALLOC_MSG_DEBUG("find_pool(" FMT_ZI ", " FMT_I ") = " FMT_I, alloc_size, flags, i);
            return pool;
        }
    }
    BADGEROS_MALLOC_MSG_DEBUG("find_pool(" FMT_ZI ", " FMT_I ") = NULL", alloc_size, flags);
    return NULL;
}

__attribute__((always_inline)) static inline size_t block_to_index(memory_pool_t *pool, buddy_block_t *block) {
    return (block - pool->blocks);
}

__attribute__((always_inline)) static inline buddy_block_t *index_to_block(memory_pool_t *pool, size_t index) {
    return &pool->blocks[index];
}

__attribute__((always_inline)) static inline void *block_to_address(memory_pool_t *pool, buddy_block_t *block) {
    size_t index = block_to_index(pool, block);
    return pool->pages_start + (index * PAGE_SIZE);
}

__attribute__((always_inline)) static inline buddy_block_t *address_to_block(memory_pool_t *pool, void *ptr) {
    return &pool->blocks[((size_t)ptr - (size_t)pool->pages_start) / PAGE_SIZE];
}

__attribute__((always_inline)) static inline size_t next_power_of_two(size_t x) {
    if (x <= 1) {
        return 1;
    }
    --x;
    x |= x >> 1;
    x |= x >> 2;
    x |= x >> 4;
    x |= x >> 8;
    x |= x >> 16;

#if __SIZEOF_SIZE_T__ == 8
    x |= x >> 32;
#endif
    return x + 1;
}

__attribute__((always_inline)) static inline uint8_t get_order(size_t size) {
    size = next_power_of_two(size);

    if (sizeof(size_t) == 8) {
        return size > 0 ? 63 - count_leading_unset_bits64(size) : 0;
    } else {
        return size > 0 ? 31 - count_leading_unset_bits32(size) : 0;
    }
}

static void list_init(buddy_block_t *list) {
    list->prev = list;
    list->next = list;
}

__attribute__((always_inline)) static inline void list_push_back(buddy_block_t *list, buddy_block_t *entry) {
    entry->in_list      = true;
    buddy_block_t *prev = list->prev;
    entry->prev         = prev;
    entry->next         = list;
    prev->next          = entry;
    list->prev          = entry;
}

__attribute__((always_inline)) static inline void list_remove(buddy_block_t *entry) {
    entry->in_list      = false;
    buddy_block_t *prev = entry->prev;
    buddy_block_t *next = entry->next;
    prev->next          = next;
    next->prev          = prev;
}

__attribute__((always_inline)) static inline bool list_empty(buddy_block_t *list) {
    if (list->prev == list) {
        return true;
    }

    return false;
}

/* Split a block
 *
 * The block we're splitting is always the left-most part, so we can just determine
 * the buddy and place that block on the free list.
 */

static void split_block(memory_pool_t *pool, buddy_block_t *block) {
    --block->order;
    size_t index       = block_to_index(pool, block);
    size_t buddy_index = index ^ (1lu << block->order); // Get buddy of our new lower order

    buddy_block_t *new_block = index_to_block(pool, buddy_index);
    new_block->order         = block->order;

    if (!new_block->is_waste) {
        list_push_back(&pool->free_lists[new_block->order], new_block); // Place buddy on the free list
    } else {
        list_push_back(&pool->waste_list, new_block); // Place buddy on the waste list
    }
}

/* Try merging a block with its buddy
 *
 * First see if our buddy is free, if it is we remove it the free list
 * and increase the order of ourselves.
 *
 * Merging only occurs when freeing a block, this means that the block we're starting
 * with is never itself on a free list.
 *
 * There are 3 possibilities here:
 *
 * - The buddy is free and is to the right of us, in this case we free our buddy,
 *   increse our order and return ourselves.
 * - The buddy is free and is to the left of us, in this case we free our buddy,
 *   switch the block pointer to our buddy, increase that order and then return that.
 * - Our buddy isn't free, we return NULL.
 */

static buddy_block_t *try_merge_buddy(memory_pool_t *pool, buddy_block_t *block) {
    size_t index       = block_to_index(pool, block);
    size_t buddy_index = index ^ (1lu << block->order);

    if (buddy_index > pool->pages + pool->max_order_waste) {
        return NULL;
    }

    buddy_block_t *buddy = index_to_block(pool, buddy_index);
    if (buddy->order == block->order && buddy->in_list) {
        // list_remove(block); // The block itself is never in a list
        list_remove(buddy);

        // Return the lowest part as the merged block.
        buddy_block_t *merged_block = index <= buddy_index ? block : buddy;
        ++merged_block->order;

        return merged_block;
    }

    return NULL;
}

/* Free a block
 *
 * In order to free a block we need to recursively try to merge the block
 * until we reach a point where there's no more free buddies to merge with.
 *
 * Finally we push the free, merged, block to our free list.
 */

void free_block(memory_pool_t *pool, buddy_block_t *block) {
    buddy_block_t *free_block = block;

    while ((block = try_merge_buddy(pool, block))) {
        free_block = block;
    }

    if (!free_block->is_waste) {
        pool->max_order_free = MAX(pool->max_order_free, free_block->order);
        list_push_back(&pool->free_lists[free_block->order], free_block);
    } else {
        list_push_back(&pool->waste_list, free_block);
    }
}

void init_pool(void *mem_start, void *mem_end, uint32_t flags) {
    if (memory_pool_num >= MAX_MEMORY_POOLS) {
        BADGEROS_MALLOC_MSG_WARN("Out of pools; discarding " FMT_P, mem_start);
        return;
    }
    size_t  total_pages = (mem_end - mem_start) / PAGE_SIZE;
    uint8_t orders      = get_order(total_pages);

    size_t metadata_block_size      = sizeof(buddy_block_t) * total_pages;
    size_t metadata_free_lists_size = sizeof(buddy_block_t) * (orders + 1);

    memory_pools[memory_pool_num].free_lists = mem_start;
    memory_pools[memory_pool_num].blocks =
        ALIGN_UP(memory_pools[memory_pool_num].free_lists + metadata_free_lists_size, 8);

    void *pages_start = ALIGN_PAGE_UP((void *)memory_pools[memory_pool_num].blocks + metadata_block_size);
    void *pages_end   = ALIGN_PAGE_DOWN(mem_end);

    size_t   pages           = ((size_t)pages_end - (size_t)pages_start) / PAGE_SIZE;
    // NOLINTNEXTLINE
    uint32_t max_order_waste = (1 << orders) - pages;

    BADGEROS_MALLOC_MSG_INFO("Initializing pool " FMT_I, memory_pool_num);
    BADGEROS_MALLOC_MSG_INFO(
        "Found " FMT_ZI " pages, usable " FMT_ZI ", overhead " FMT_ZI,
        total_pages,
        pages,
        total_pages - pages
    );
    BADGEROS_MALLOC_MSG_INFO("Mem start: " FMT_P ", pages_start, " FMT_P, mem_start, pages_start);
    BADGEROS_MALLOC_MSG_INFO("Mem end: " FMT_P ", pages_end, " FMT_P, mem_end, pages_end);
    BADGEROS_MALLOC_MSG_INFO("Max orders: " FMT_I ", max_order_waste: " FMT_I, orders, max_order_waste);
    BADGEROS_MALLOC_MSG_INFO("Waste starts at: " FMT_ZI, pages - max_order_waste);
    BADGEROS_MALLOC_MSG_INFO("Metadata block size: " FMT_ZI, metadata_block_size);
    BADGEROS_MALLOC_MSG_INFO("Metadata free lists size: " FMT_ZI, metadata_free_lists_size);

    memory_pools[memory_pool_num].flags           = flags;
    memory_pools[memory_pool_num].start           = mem_start;
    memory_pools[memory_pool_num].end             = mem_end;
    memory_pools[memory_pool_num].pages_start     = pages_start;
    memory_pools[memory_pool_num].pages_end       = pages_end;
    memory_pools[memory_pool_num].pages           = pages;
    memory_pools[memory_pool_num].free_pages      = pages;
    memory_pools[memory_pool_num].max_order       = orders;
    memory_pools[memory_pool_num].max_order_waste = max_order_waste;

    // Zero out all of our metadata
    __builtin_memset(memory_pools[memory_pool_num].start, 0, pages_start - mem_start); // NOLINT

    // Initialize our free lists to be empty
    for (int i = 0; i <= orders; ++i) {
        list_init(&memory_pools[memory_pool_num].free_lists[i]);
    }
    list_init(&memory_pools[memory_pool_num].waste_list);

    // Mark all of our waste pages as unusable
    for (size_t i = pages - max_order_waste; i < total_pages; ++i) {
        memory_pools[memory_pool_num].blocks[i].is_waste = true;
    }

    // Create free block of all available pages
    memory_pools[memory_pool_num].blocks[0].order = orders;
    memory_pools[memory_pool_num].blocks[0].next  = &memory_pools[memory_pool_num].blocks[0];
    memory_pools[memory_pool_num].blocks[0].prev  = &memory_pools[memory_pool_num].blocks[0];

    // Push free block to the free list
    list_push_back(&memory_pools[memory_pool_num].free_lists[orders], &memory_pools[memory_pool_num].blocks[0]);
    memory_pools[memory_pool_num].max_order_free = orders;
    ++memory_pool_num;
}

#ifndef BADGEROS_KERNEL
void print_list(memory_pool_t *pool, buddy_block_t *list, size_t *total) {
    size_t         blocks     = 0;
    size_t         list_total = 0;
    buddy_block_t *block      = list;
    while (block) {
        if (block->prev == list) {
            break;
        }
        ++blocks;
        block       = block->prev;
        list_total += 1lu << block->order;
        printf("(%zi) ", block_to_index(pool, block));
    }
    *total += list_total;
    printf("%zu blocks (%zi pages)\n", blocks, list_total);
}

void print_allocator() {
    for (int p = 0; p < memory_pool_num; ++p) {
        memory_pool_t *pool = &memory_pools[p];
        printf("Pool %i: \n", p);

        size_t total = 0;
        for (int i = 0; i <= pool->max_order; ++i) {
            printf("Order %i, ", i);
            print_list(pool, &pool->free_lists[i], &total);
        }

        printf("Waste: ");
        print_list(pool, &pool->waste_list, &total);

        printf(
            "Total free pages: (calculated) %zi (stored) %zi max_order_free: %i\n",
            total - pool->max_order_waste,
            pool->free_pages,
            pool->max_order_free
        );
    }
}
#endif

/* Find a suitable block
 *
 * We start by looking at the first block of the appropriate order, if we get a block
 * we validate that the allocation of the desired number of pages doesn't go into
 * a waste page. If it does we try all other pages of that order until we find one
 * that will suit our needs.
 */

__attribute__((always_inline)) static inline buddy_block_t *
    pool_find_block(memory_pool_t *pool, uint8_t allocation_order, size_t pages) {
    for (uint8_t a = allocation_order; a <= pool->max_order; ++a) {
        buddy_block_t *list  = &pool->free_lists[a];
        buddy_block_t *block = list;

        while (block->prev != list) {
            block                             = block->prev;
            buddy_block_t *request_last_block = index_to_block(pool, (block_to_index(pool, block) + pages) - 1);

            if (!request_last_block->is_waste) {
                list_remove(block);
                return block;
            }
        }
    }

    return NULL;
}

/* Allocation
 *
 * Allocation works by finding the smallest possible block that can satisfy
 * the allocation request (in pages). This block might be too big.
 *
 * If the block is too big we split it by lowering the order of the block
 * we got, and then placing its buddy on the free list of that order.
 *
 * We split in a loop until we have a block of the appropriate size. Splitting
 * all the way to the size we need, but never any smaller.
 */

void *buddy_allocate(size_t size, enum block_type type, uint32_t flags) {
    (void)flags;
    BADGEROS_MALLOC_MSG_DEBUG("buddy_allocate(" FMT_ZI ")", size);
    if (!size) {
        return NULL;
    }

    size_t         pages                     = (size + (PAGE_SIZE - 1)) / PAGE_SIZE;
    uint8_t        allocation_order          = get_order(pages);
    uint8_t        original_allocation_order = allocation_order;
    buddy_block_t *block                     = NULL;
    memory_pool_t *pool                      = NULL;

    BADGEROS_MALLOC_MSG_DEBUG(
        "buddy_allocate(" FMT_ZI ") allocating " FMT_ZI " pages, order " FMT_I,
        size,
        pages,
        allocation_order
    );

    for (int i = 0; i < memory_pool_num; ++i) {
        pool = find_pool(i, allocation_order, pages, 0);
        if (!pool) {
            break;
        }

        if (allocation_order == pool->max_order) {
            // NOLINTNEXTLINE
            if (size > (1 << allocation_order) - pool->max_order_waste) {
                BADGEROS_MALLOC_MSG_WARN("buddy_allocate(" FMT_ZI ") = NULL (Allocation too large)", size);
                continue;
            }
        }

        block = pool_find_block(pool, allocation_order, pages);
        if (block)
            break;
    }

    if (!pool || !block) {
        BADGEROS_MALLOC_MSG_WARN("buddy_allocate(" FMT_ZI ") = NULL (OOM) no pool", size);
        return NULL;
    }

    if (block->order != original_allocation_order) {
        while (block->order > original_allocation_order) {
            split_block(pool, block);
        }
    }

    while (pool->max_order_free && list_empty(&pool->free_lists[pool->max_order_free])) {
        if (pool->max_order_free)
            --pool->max_order_free;
    }

    pool->free_pages -= (1lu << block->order);
    block->type       = type;
    void *retval      = block_to_address(pool, block);

    BADGEROS_MALLOC_MSG_DEBUG("buddy_allocate(" FMT_ZI ") returning " FMT_P, size, retval);
    return retval;
}

__attribute__((always_inline)) static inline buddy_block_t *buddy_get_block(void *ptr, memory_pool_t **pool) {
    BADGEROS_MALLOC_MSG_DEBUG("buddy_get_block(" FMT_P ")", ptr);
    if (!ptr) {
        return NULL;
    }

    void *aligned_ptr = ALIGN_PAGE_DOWN(ptr);
    if (aligned_ptr != ptr) {
        BADGEROS_MALLOC_MSG_ERROR("buddy_get_block(" FMT_P ") = Pointer not page aligned", ptr);
        panic_abort();
        return NULL;
    }

    *pool = ptr_to_pool(ptr);
    if (!*pool) {
        BADGEROS_MALLOC_MSG_ERROR("buddy_get_block(" FMT_P ") = Pointer not in a pool", ptr);
        panic_abort();
        return NULL;
    }

    buddy_block_t *block = address_to_block(*pool, ptr);
    return block;
}

/* Deallocation
 *
 * We deallocate by recursively checking for each block whether its buddy is free.
 * If the buddy is free (that is, it is on our free list) then we remove the buddy
 * from the free list and increase our order (doubling our size) until there
 * are no more buddies free.
 *
 * This means that at any time we have the largest possible allocation available.
 */

void buddy_deallocate(void *ptr) {
    BADGEROS_MALLOC_MSG_DEBUG("buddy_deallocate(" FMT_P ")", ptr);

    memory_pool_t *pool  = NULL;
    buddy_block_t *block = buddy_get_block(ptr, &pool);

    if (!block) {
        return;
    }

#ifndef NDEBUG
    // Fill with dummy value in debug builds to catch UAF.
    mem_set(ptr, 0xec, CONFIG_PAGE_SIZE << block->order);
#endif

    pool->free_pages += (1lu << block->order);
    block->type       = BLOCK_TYPE_FREE;
    free_block(pool, block);
}

void buddy_split_allocated(void *ptr) {
    memory_pool_t *pool  = NULL;
    buddy_block_t *block = buddy_get_block(ptr, &pool);

    if (!block) {
        return;
    }

    --block->order;
    size_t index       = block_to_index(pool, block);
    size_t buddy_index = index ^ (1lu << block->order); // Get buddy of our new lower order

    buddy_block_t *new_block = index_to_block(pool, buddy_index);
    new_block->order         = block->order;
    new_block->type          = block->type;
}

void *buddy_reallocate(void *ptr, size_t size) {
    BADGEROS_MALLOC_MSG_DEBUG("buddy_reallocate(" FMT_P ", " FMT_ZI ")", ptr, size);

    memory_pool_t *pool  = NULL;
    buddy_block_t *block = buddy_get_block(ptr, &pool);

    if (!block) {
        return NULL;
    }

    size_t  pages            = (size + (PAGE_SIZE - 1)) / PAGE_SIZE;
    uint8_t allocation_order = get_order(pages);
    size_t  old_size         = (1lu << block->order) * PAGE_SIZE;

    if (block->order == allocation_order) {
        BADGEROS_MALLOC_MSG_DEBUG("buddy_reallocate(" FMT_P ", " FMT_ZI ") nothing to do", ptr, size);
        return ptr;
    }

    void *new_block = buddy_allocate(size, block->type, pool->flags);
    if (!new_block) {
        BADGEROS_MALLOC_MSG_WARN("buddy_reallocate(" FMT_P ", " FMT_ZI ") couldn't allocate new block", ptr, size);
        return NULL;
    }

    size_t copy_size = old_size < size ? old_size : size;

    __builtin_memcpy(new_block, ptr, copy_size); // NOLINT
    buddy_deallocate(ptr);

    BADGEROS_MALLOC_MSG_DEBUG("buddy_reallocate(" FMT_P ", " FMT_ZI ") returning " FMT_P, ptr, size, new_block);
    return new_block;
}

enum block_type buddy_get_type(void *ptr) {
    BADGEROS_MALLOC_MSG_DEBUG("buddy_get_type(" FMT_P ")", ptr);

    memory_pool_t *pool  = NULL;
    buddy_block_t *block = buddy_get_block(ptr, &pool);

    if (!block) {
        return BLOCK_TYPE_ERROR;
    }

    return block->type;
}

size_t buddy_get_size(void *ptr) {
    BADGEROS_MALLOC_MSG_DEBUG("buddy_get_size(" FMT_P ")", ptr);

    memory_pool_t *pool  = NULL;
    buddy_block_t *block = buddy_get_block(ptr, &pool);

    if (!block) {
        return 0;
    }

    BADGEROS_MALLOC_MSG_DEBUG("buddy_get_size(" FMT_P ") returning " FMT_I, ptr, (1lu << block->order) * PAGE_SIZE);
    return (1lu << block->order) * PAGE_SIZE;
}
