// SPDX-License-Identifier: MIT

/* Slab allocator for BadgerOS
 *
 * A Slab allocator works by dividing some memory region into slabs of identical size.
 * In our case, we support slabs of 32, 64, 128, and 256 bytes. Each page has its own
 * bitmap of free slots in the slab.
 *
 * Furthermore the allocator keeps track of how full each slab is, and tries to use
 * the fullest slab first. The idea being that fuller slabs are less likely to be fully
 * empty sooner than slabs that are almost empty.
 *
 * Once a page is fully empty, and there are no other empty pages of that size free, we
 * return the page to the global memory pool.
 */

#ifndef BADGEROS_KERNEL
#define _GNU_SOURCE
#endif

#include "badge_strings.h"
#include "bitops.h"
#include "debug.h"
#include "static-buddy.h"

#include <stdbool.h>
#include <stdint.h>

#define BITMAP_WORDS        4
#define DATA_OFFSET         64
#define PAGE_ALLOC_BITS(x)  (((PAGE_SIZE - 32) - ((x) - 1)) / (x))
#define PAGE_ALLOC_BYTES(x) ((PAGE_ALLOC_BITS(x) + 7) / 8)

enum slab_sizes_t { SLAB_SIZE_32 = 0, SLAB_SIZE_64 = 1, SLAB_SIZE_128 = 2, SLAB_SIZE_256 = 3 };
enum slab_use_t {
    SLAB_USE_FULL         = 4,
    SLAB_USE_NEAR_FULL    = 3,
    SLAB_USE_HALF_FULL    = 2,
    SLAB_USE_ALMOST_EMPTY = 1,
    SLAB_USE_EMPTY        = 0
};

// Bytes per slab allocation
static uint16_t slab_bytes[]        = {32, 64, 128, 256};
static uint16_t slab_tresholds[][4] = {
    {0, 32, 64, 126},
    {0, 16, 32, 63},
    {0, 8, 16, 31},
    {0, 4, 8, 15},
};

// This needs to be correct otherwise finding an empty slab slot will not work
static uint32_t slab_empty[][4] = {
    {UINT32_MAX, UINT32_MAX, UINT32_MAX, 0x3FFFFFFF},
    {UINT32_MAX, 0x7FFFFFFF, 0, 0},
    {0x7FFFFFFF, 0, 0, 0},
    {0x00007FFF, 0, 0, 0}
};

typedef struct slab_header_t {
    uint8_t               size;
    uint8_t               use_count;
    struct slab_header_t *prev;
    struct slab_header_t *next;
    enum slab_use_t       slab_use;
    uint32_t              bitmap[4];
} slab_header_t;

typedef struct {
    // 4 Full
    // 3 Near full
    // 2 Half full
    // 1 < Quarter full
    // 0 Empty
    slab_header_t slabs[5];
} slab_lists_t;

static slab_lists_t slabs[4];

__attribute__((always_inline)) static inline uint32_t bitmap_clear_bit(uint32_t const word, uint8_t bit_index) {
    BADGEROS_MALLOC_ASSERT_ERROR(bit_index <= 31, "bit_index out of range " FMT_I " > 31", bit_index);
    return word & ~((uint32_t)1 << bit_index);
}

static inline uint32_t bitmap_set_bit(uint32_t const word, uint8_t bit_index) {
    BADGEROS_MALLOC_ASSERT_ERROR(bit_index <= 31, "bit_index out of range " FMT_I " > 31", bit_index);
    return word | ((uint32_t)1 << bit_index);
}

static void list_init(slab_header_t *list) {
    list->prev = list;
    list->next = list;
}

__attribute__((always_inline)) static inline void list_push_back(slab_header_t *list, slab_header_t *entry) {
    slab_header_t *prev = list->prev;
    entry->prev         = prev;
    entry->next         = list;
    prev->next          = entry;
    list->prev          = entry;
}

__attribute__((always_inline)) static inline void list_remove(slab_header_t *entry) {
    slab_header_t *prev = entry->prev;
    slab_header_t *next = entry->next;
    prev->next          = next;
    next->prev          = prev;
}

__attribute__((always_inline)) static inline bool list_empty(slab_header_t *list) {
    if (list->prev == list) {
        return true;
    }

    return false;
}

// Initialize a page and turn it into a slab page
__attribute__((always_inline)) static inline void init_slab(slab_header_t *slab, enum slab_sizes_t size) {
    for (uint32_t i = 0; i < BITMAP_WORDS; ++i) {
        slab->bitmap[i] = slab_empty[size][i];
    }

    slab->size      = size;
    slab->use_count = 1;
    slab->slab_use  = SLAB_USE_ALMOST_EMPTY;
}

// Find a slab page with free space in it, if it cannot be found we allocate a new page.
// We start off by looking at pages that are close to full, working our way down until we find
// the fullest page that still has some space in it.
__attribute__((always_inline)) static inline slab_header_t *get_slab(enum slab_sizes_t size) {
    BADGEROS_MALLOC_MSG_DEBUG("get_slab(" FMT_I ")", size);
    slab_header_t *slab;

    for (int i = SLAB_USE_NEAR_FULL; i >= SLAB_USE_EMPTY; --i) {
        if (!list_empty(&slabs[size].slabs[i])) {
            slab = slabs[size].slabs[i].prev;
            ++slab->use_count;

            if (slab->use_count >= slab_tresholds[size][i]) {
                // This slab page has gone over the treshold, move it up to a list of fuller pages
                list_remove(slab);
                list_push_back(&slabs[size].slabs[i + 1], slab);
                ++slab->slab_use;
            }

            BADGEROS_MALLOC_MSG_DEBUG("get_slab(" FMT_I ") returning " FMT_P, size, slab);
            return slab;
        }
    }

    BADGEROS_MALLOC_MSG_DEBUG("get_slab(" FMT_I ") allocation new page", size);

    uint16_t pages      = 1;
    void    *allocation = buddy_allocate(pages * PAGE_SIZE, BLOCK_TYPE_SLAB, 0);

    if (!allocation) {
        BADGEROS_MALLOC_MSG_DEBUG("get_slab(" FMT_I ") allocation failed, returning NULL", size);
        return NULL;
    }

    for (int i = 0; i < pages; ++i) {
        slab = allocation + (i * PAGE_SIZE);
        init_slab(slab, size);
        list_push_back(&slabs[size].slabs[SLAB_USE_ALMOST_EMPTY], slab);
    }

    BADGEROS_MALLOC_MSG_DEBUG("get_slab(" FMT_I ") returning " FMT_P, size, slab);
    return slab;
}

// Initialize the kernel's slab lists
void init_kernel_slabs() {
    BADGEROS_MALLOC_MSG_DEBUG("init_kernel_slabs()");
    for (int i = 0; i < 4; ++i) {
        for (int k = 0; k < 5; ++k) {
            list_init(&slabs[i].slabs[k]);
        }
    }
}

void *slab_allocate(size_t size, enum slab_type type, uint32_t flags) {
    (void)type;
    (void)flags;
    BADGEROS_MALLOC_MSG_DEBUG("slab_allocate(" FMT_ZI ")", size);
    if (size > 256)
        return NULL;

    uint8_t slab_type = SLAB_SIZE_32;
    if (size > 128) {
        slab_type = SLAB_SIZE_256;
    } else if (size > 64) {
        slab_type = SLAB_SIZE_128;
    } else if (size > 32) {
        slab_type = SLAB_SIZE_64;
    }

    slab_header_t *page = NULL;
    page                = get_slab(slab_type);
    if (!page) {
        BADGEROS_MALLOC_MSG_DEBUG("slab_allocate(" FMT_ZI ") No page, returning NULL", size);
        return NULL;
    }

    for (uint32_t i = 0; i < BITMAP_WORDS; ++i) {
        if (!page->bitmap[i])
            continue;

        uint32_t bit_index = find_first_trailing_set_bit32(page->bitmap[i]);
        page->bitmap[i]    = bitmap_clear_bit(page->bitmap[i], bit_index);

        size_t index  = (i * 32) + bit_index;
        void  *retval = ((uint8_t *)page) + DATA_OFFSET + (index * slab_bytes[slab_type]);
        BADGEROS_MALLOC_MSG_DEBUG("slab_allocate(" FMT_ZI ") returning " FMT_P, size, retval);
        return retval;
    }

    BADGEROS_MALLOC_MSG_ERROR("slab_allocate(" FMT_ZI ") Slab didn't have a free slot, returning NULL", size);
    return NULL;
}

void slab_deallocate(void *ptr) {
    BADGEROS_MALLOC_MSG_DEBUG("slab_deallocate(" FMT_P ")", ptr);
    if (!ptr) {
        BADGEROS_MALLOC_MSG_DEBUG("slab_deallocate(" FMT_P ") = NULL", ptr);
        return;
    }

    slab_header_t *header     = ALIGN_PAGE_DOWN(ptr);
    size_t         offset     = (size_t)(ptr) - (size_t)(header);
    offset                   -= DATA_OFFSET;
    uint32_t total_bit_index  = offset / slab_bytes[header->size];
    uint32_t word_index       = total_bit_index / 32;
    uint32_t bit_index        = total_bit_index % 32;
    uint32_t new_bitmap       = bitmap_set_bit(header->bitmap[word_index], bit_index);

    if (header->bitmap[word_index] == new_bitmap) {
        BADGEROS_MALLOC_MSG_ERROR("slab_deallocate(" FMT_P ") = Double free", ptr);
        return;
    }

#ifndef NDEBUG
    // Fill with dummy value in debug builds to catch UAF.
    mem_set(ptr, 0xec, slab_bytes[header->size]);
#endif

    header->bitmap[word_index] = new_bitmap;
    --header->use_count;

    // Check to see if the slab page is still in the correct bucket. If it is not move it to
    // an emptier bucket
    if (header->use_count <= slab_tresholds[header->size][header->slab_use - 1]) {
        header->slab_use -= 1;
        list_remove(header);
        if (header->slab_use > SLAB_USE_EMPTY || list_empty(&slabs[header->size].slabs[header->slab_use])) {
            list_push_back(&slabs[header->size].slabs[header->slab_use], header);
        } else {
            // This slab page is empty, and we have another empty one
            buddy_deallocate(header);
        }
    }
}

size_t slab_get_size(void *ptr) {
    BADGEROS_MALLOC_MSG_DEBUG("slab_get_size(" FMT_P ")", ptr);
    if (!ptr) {
        BADGEROS_MALLOC_MSG_DEBUG("slab_get_size(" FMT_P ") = NULL", ptr);
        return 0;
    }

    slab_header_t *header = ALIGN_PAGE_DOWN(ptr);
    BADGEROS_MALLOC_MSG_DEBUG("slab_get_size(" FMT_P ") returning " FMT_I, ptr, slab_bytes[header->size]);
    return slab_bytes[header->size];
}
