#include "static-buddy.h"

#include <stdio.h>
#include <stdlib.h>

#include <time.h>

#define PAGE_SIZE   4096
#define MEMORY_SIZE PAGE_SIZE * 65536

int main() {
    srand(time(NULL));
    char *ram  = malloc(MEMORY_SIZE);
    char *ram2 = malloc(MEMORY_SIZE);

    init_pool(ram, ram + MEMORY_SIZE, 0);
    init_pool(ram2, ram2 + MEMORY_SIZE, 0);
    init_kernel_slabs();

    // for(int j = 0; j < 1024; ++j) {
    char **allocations = calloc(1, sizeof(void *) * (MEMORY_SIZE / PAGE_SIZE) * 2);

    size_t i     = 0, k;
    size_t alloc = 0, dealloc = 0;
    for (; i < (MEMORY_SIZE / PAGE_SIZE * 2); ++i) {
        allocations[i] = buddy_allocate(PAGE_SIZE * ((rand() % 10) + 1), BLOCK_TYPE_PAGE, 0);
        if (!allocations[i]) {
            break;
        }
        ++alloc;
    }

    // for (size_t k = i; k >= 0; --k) {
    for (k = 0; k <= i / 2; ++k) {
        if (allocations[k])
            ++dealloc;
        buddy_deallocate(allocations[k]);
        allocations[k] = NULL;
    }
    // print_allocator();

    for (i = k; i < (MEMORY_SIZE / PAGE_SIZE * 2); ++i) {
        if (allocations[i])
            continue;

        allocations[i] = buddy_allocate(PAGE_SIZE * ((rand() % 10) + 1), BLOCK_TYPE_PAGE, 0);
        if (!allocations[i]) {
            break;
        }
        ++alloc;
    }

    for (k = 0; k < MEMORY_SIZE / PAGE_SIZE * 2; ++k) {
        if (allocations[k])
            ++dealloc;
        buddy_deallocate(allocations[k]);
        allocations[k] = NULL;
    }

    for (int p = 0; p < memory_pool_num; ++p) {
        memory_pool_t *pool = &memory_pools[p];

        if (pool->free_lists[pool->max_order].prev == &pool->free_lists[pool->max_order]) {
            print_allocator();
            printf("Didn't free all pages\n");
            return 1;
        }
    }

#define SLAB_ALLOCATIONS (MEMORY_SIZE / 128) * 2
    char **slab_allocations = calloc(1, sizeof(void *) * SLAB_ALLOCATIONS);
    int    slab_sizes[]     = {32, 64, 128, 256};

    for (int i = 0; i < SLAB_ALLOCATIONS; ++i) {
        int slab_size       = slab_sizes[abs(rand() % 4)];
        slab_allocations[i] = slab_allocate(slab_size, SLAB_TYPE_SLAB, 0);
    }

    for (int i = SLAB_ALLOCATIONS - 1; i >= 0; --i) {
        slab_deallocate(slab_allocations[i]);
    }

    slab_deallocate(slab_allocations[0]);

    for (int p = 0; p < memory_pool_num; ++p) {
        memory_pool_t *pool = &memory_pools[p];

        if (pool->free_pages < pool->pages - 4) {
            print_allocator();
            printf("Didn't free all pages\n");
            return 1;
        }
    }

    print_allocator();
    // printf("Did %zu allocations and %zu deallocations\n", alloc, dealloc);
    free(allocations);
    free(slab_allocations);
    //}
    free(ram);
    free(ram2);
}
