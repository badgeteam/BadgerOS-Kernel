
// SPDX-License-Identifier: MIT

#pragma once

#include "scheduler/waitlist.h"
#include "time.h"

#include <stdatomic.h>
#include <stdbool.h>

// Semaphore.
typedef struct {
    // Number of times posted.
    atomic_int available;
    // Threads waiting for the semaphore to be posted.
    waitlist_t waiting_list;
} sem_t;

#define SEM_FAST_LOOPS 256
#define SEM_T_INIT     {0, WAITLIST_T_INIT}

// Initialize a new semaphore.
void sem_init(sem_t *sem);
// Clean up a semaphore.
void sem_destroy(sem_t *sem);

// Reset the semaphore count.
void sem_reset(sem_t *sem);
// Post the semaphore once.
void sem_post(sem_t *sem);
// Await the semaphore.
bool sem_await(sem_t *sem, timestamp_us_t timeout);
