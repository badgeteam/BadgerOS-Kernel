
// SPDX-License-Identifier: MIT

#ifndef BADGEOS_KERNEL
// NOLINTNEXTLINE
#define _GNU_SOURCE
#endif

#include "debug.h"
#include "spinlock.h"
#include "static-buddy.h"

#include <stddef.h>
#include <stdint.h>

#ifdef BADGEROS_KERNEL
// NOLINTBEGIN
extern char __start_free_sram[];
extern char __stop_free_sram[];

#define __wrap_malloc         malloc
#define __wrap_free           free
#define __wrap_calloc         calloc
#define __wrap_realloc        realloc
#define __wrap_reallocarray   reallocarray
#define __wrap_aligned_alloc  aligned_alloc
#define __wrap_posix_memalign posix_memalign
// NOLINTEND

#else
#include <stdio.h>
#include <stdlib.h>

#include <unistd.h>

// NOLINTBEGIN
void *__real_malloc(size_t size);
void  __real_free(void *ptr);
void *__real_calloc(size_t nmemb, size_t size);
void *__real_realloc(void *ptr, size_t size);
void *__real_reallocarray(void *ptr, size_t nmemb, size_t size);
// NOLINTEND
#endif

#ifdef PRELOAD
// NOLINTBEGIN
#define __wrap_malloc         malloc
#define __wrap_free           free
#define __wrap_calloc         calloc
#define __wrap_realloc        realloc
#define __wrap_reallocarray   reallocarray
#define __wrap_aligned_alloc  aligned_alloc
#define __wrap_posix_memalign posix_memalign
// NOLINTEND
#endif

static bool        mem_initialized = false;
static atomic_flag lock            = ATOMIC_FLAG_INIT;

void kernel_heap_init();

void kernel_heap_init() {
#ifdef BADGEROS_KERNEL
#ifndef CONFIGENUM_SOC_generic
    init_pool(__start_free_sram, __stop_free_sram, 0);
#endif
    init_kernel_slabs();
#else
    SPIN_LOCK_LOCK(lock);
    if (mem_initialized) {
        SPIN_LOCK_UNLOCK(lock);
        return;
    }

    sbrk(1024 * 1024); // buffer
    void *mem_start = sbrk(3221225472);
    void *mem_end   = sbrk(0);
    init_pool(mem_start, mem_end, 0);
    init_kernel_slabs();
    SPIN_LOCK_UNLOCK(lock);
#endif

    mem_initialized = true;
}

// NOLINTNEXTLINE
void *_malloc(size_t size) {
    BADGEROS_MALLOC_MSG_DEBUG("malloc(" FMT_ZI ")", size);
    if (!size)
        size = 1;

    if (size <= MAX_SLAB_SIZE) {
        return slab_allocate(size, SLAB_TYPE_SLAB, 0);
    }
    return buddy_allocate(size, BLOCK_TYPE_PAGE, 0);
}

// NOLINTNEXTLINE
void *__wrap_malloc(size_t size) {
#ifdef PRELOAD
    if (!mem_initialized)
        kernel_heap_init();
#endif
    SPIN_LOCK_LOCK(lock);
    void *ptr = _malloc(size);
    SPIN_LOCK_UNLOCK(lock);

    return ptr;
}

// NOLINTNEXTLINE
void *__wrap_aligned_alloc(size_t alignment, size_t size) {
    (void)alignment;
#ifdef PRELOAD
    if (!mem_initialized)
        kernel_heap_init();
#endif
    SPIN_LOCK_LOCK(lock);
    void *ptr = _malloc(size);
    SPIN_LOCK_UNLOCK(lock);

    return ptr;
}

// NOLINTNEXTLINE
int __wrap_posix_memalign(void **memptr, size_t alignment, size_t size) {
    (void)alignment;
#ifdef PRELOAD
    if (!mem_initialized)
        kernel_heap_init();
#endif
    SPIN_LOCK_LOCK(lock);
    void *ptr = _malloc(size);
    SPIN_LOCK_UNLOCK(lock);

    *memptr = ptr;
    return 0;
}

// NOLINTNEXTLINE
void *__wrap_calloc(size_t nmemb, size_t size) {
#ifdef PRELOAD
    if (!mem_initialized)
        kernel_heap_init();
#endif

    SPIN_LOCK_LOCK(lock);
    void *ptr = _malloc(nmemb * size);
    if (ptr)
        __builtin_memset(ptr, 0, nmemb * size); // NOLINT
    SPIN_LOCK_UNLOCK(lock);
    return ptr;
}

// NOLINTNEXTLINE
static void _free(void *ptr) {
    BADGEROS_MALLOC_MSG_DEBUG("free(" FMT_P ")", ptr);
    if (!ptr) {
        return;
    }

    enum block_type type = buddy_get_type(ALIGN_PAGE_DOWN(ptr));

    switch (type) {
        case BLOCK_TYPE_PAGE: buddy_deallocate(ptr); break;
        case BLOCK_TYPE_SLAB: slab_deallocate(ptr); break;
        default: BADGEROS_MALLOC_MSG_ERROR("free(" FMT_P ") = Unknown pointer type", ptr);
    }
}

// NOLINTNEXTLINE
void __wrap_free(void *ptr) {
#ifdef PRELOAD
    if (!mem_initialized)
        kernel_heap_init();
#endif

    SPIN_LOCK_LOCK(lock);
    _free(ptr);
    SPIN_LOCK_UNLOCK(lock);
}

// NOLINTNEXTLINE
void *__wrap_realloc(void *ptr, size_t size) {
#ifdef PRELOAD
    if (!mem_initialized)
        kernel_heap_init();
#endif
    BADGEROS_MALLOC_MSG_DEBUG("realloc(" FMT_P ", " FMT_ZI ")", ptr, size);

    if (!ptr) {
        return __wrap_malloc(size);
    }

    if (!size) {
        __wrap_free(ptr);
        return NULL;
    }

    size_t old_size = 0;

    SPIN_LOCK_LOCK(lock);
    enum block_type type = buddy_get_type(ALIGN_PAGE_DOWN(ptr));
    switch (type) {
        case BLOCK_TYPE_PAGE: old_size = buddy_get_size(ptr); break;
        case BLOCK_TYPE_SLAB: old_size = slab_get_size(ptr); break;
        default:
            BADGEROS_MALLOC_MSG_ERROR("realloc(" FMT_P ") = Unknown pointer type: " FMT_I, ptr, type);
            SPIN_LOCK_UNLOCK(lock);
            return ptr;
    }

    char *new_ptr = NULL;

    if (old_size >= size) {
        if (old_size > MAX_SLAB_SIZE && size > MAX_SLAB_SIZE) {
            new_ptr = buddy_reallocate(ptr, size);
            SPIN_LOCK_UNLOCK(lock);
            return new_ptr;
        }
    }

    new_ptr = _malloc(size);
    if (!new_ptr) {
        BADGEROS_MALLOC_MSG_WARN("realloc: failed to allocate memory, returning NULL");
        SPIN_LOCK_UNLOCK(lock);
        return NULL;
    }

    size_t copy_size = old_size < size ? old_size : size;
    __builtin_memcpy(new_ptr, ptr, copy_size); // NOLINT
    switch (type) {
        case BLOCK_TYPE_PAGE: buddy_deallocate(ptr); break;
        case BLOCK_TYPE_SLAB: slab_deallocate(ptr); break;
        default: BADGEROS_MALLOC_MSG_ERROR("realloc(" FMT_P ") = Unknown pointer type", ptr);
    }
    SPIN_LOCK_UNLOCK(lock);

    return new_ptr;
}

// NOLINTNEXTLINE
void *__wrap_reallocarray(void *ptr, size_t nmemb, size_t size) {
    return __wrap_realloc(ptr, nmemb * size);
}
