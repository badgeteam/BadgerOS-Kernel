
// SPDX-License-Identifier: MIT

#pragma once

#include <stdatomic.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// Generic reference counting struct.
struct rc_t {
    // Number of references to the data.
    atomic_int refcount;
    // Pointer to the data.
    void      *data;
    // Cleanup function for the data.
    void (*cleanup)(void *);
};

// Generic reference counting pointer.
typedef struct rc_t *rc_t;



// Create a new refcount pointer, return NULL if out of memory.
rc_t rc_new(void *data, void (*cleanup)(void *));
// Create a new refcount pointer, abort if out of memory.
rc_t rc_new_strong(void *data, void (*cleanup)(void *));
// Take a new share from a refcount pointer.
rc_t rc_share(rc_t rc);
// Delete a share from a refcount pointer.
void rc_delete(rc_t rc);
