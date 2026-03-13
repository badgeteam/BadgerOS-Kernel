
// SPDX-License-Identifier: MIT

#include "refcount.h"

#include <malloc.h>

#ifdef BADGEROS_KERNEL

#include <log.h>
#include <panic.h>

static inline void rc_abort(void *ptr, int value) {
    logkf(LOG_FATAL, "Refcount pointer 0x%{size;x} has invalid share count %{d}", ptr, value);
    panic_abort();
}

static inline void rc_strong_fail() {
    logk(LOG_FATAL, "Out of memory to create refcount pointer");
    panic_abort();
}

#else

#include <stdio.h>
#include <stdlib.h>

static inline void rc_abort(void *ptr, int value) {
    printf("Refcount pointer %p has invalid share count %d\n", ptr, value);
    abort();
}

static inline void rc_strong_fail() {
    printf("Out of memory to create refcount pointer\n");
    abort();
}

#endif



// Create a new refcount pointer, return NULL if out of memory.
rc_t rc_new(void *data, void (*cleanup)(void *)) {
    rc_t rc = malloc(sizeof(struct rc_t));
    if (!rc) {
        return NULL;
    }
    rc->data    = data;
    rc->cleanup = cleanup;
    atomic_store_explicit(&rc->refcount, 1, memory_order_release);
    return rc;
}

// Create a new refcount pointer, abort if out of memory.
rc_t rc_new_strong(void *data, void (*cleanup)(void *)) {
    rc_t rc = rc_new(data, cleanup);
    if (!rc) {
        rc_strong_fail();
    }
    return rc;
}

// Take a new share from a refcount pointer.
rc_t rc_share(rc_t rc) {
    int prev = atomic_fetch_add(&rc->refcount, 1);
    if (prev <= 0 || prev == __INT_MAX__) {
        rc_abort(rc, prev);
    }
    return rc;
}

// Delete a share from a refcount pointer.
void rc_delete(rc_t rc) {
    int prev = atomic_fetch_sub(&rc->refcount, 1);
    if (prev <= 0) {
        rc_abort(rc, prev);
    } else if (prev == 1) {
        if (rc->cleanup) {
            rc->cleanup(rc->data);
        }
        free(rc);
    }
}
