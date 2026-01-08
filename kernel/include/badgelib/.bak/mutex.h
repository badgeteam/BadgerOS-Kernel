
// SPDX-License-Identifier: MIT

#pragma once

#include "scheduler/waitlist.h"
#include "time.h"

#include <stdatomic.h>
#include <stdbool.h>

// A mutex.
typedef struct {
    // Mutex allows sharing.
    bool       is_shared;
    // Share count and/or is locked.
    atomic_int shares;
    // List of threads waiting for this mutex.
    waitlist_t waiting_list;
} mutex_t;


#define MUTEX_FAST_LOOPS    256
#define MUTEX_T_INIT        {0, 0, WAITLIST_T_INIT}
#define MUTEX_T_INIT_SHARED {1, 0, WAITLIST_T_INIT}



// Recommended way to create a mutex at run-time.
void mutex_init(mutex_t *mutex, bool shared);
// Clean up the mutex.
void mutex_destroy(mutex_t *mutex);

// Try to acquire `mutex` within `max_wait_us` microseconds.
// If `max_wait_us` is too long or negative, do not use the timeout.
// Returns true if the mutex was successully acquired.
bool mutex_acquire(mutex_t *mutex, timestamp_us_t max_wait_us);
// Release `mutex`, if it was initially acquired by this thread.
// Returns true if the mutex was successfully released.
bool mutex_release(mutex_t *mutex);

// Try to acquire a share in `mutex` within `max_wait_us` microseconds.
// If `max_wait_us` is too long or negative, do not use the timeout.
// Returns true if the share was successfully acquired.
bool mutex_acquire_shared(mutex_t *mutex, timestamp_us_t max_wait_us);
// Release `mutex`, if it was initially acquired by this thread.
// Returns true if the mutex was successfully released.
bool mutex_release_shared(mutex_t *mutex);
