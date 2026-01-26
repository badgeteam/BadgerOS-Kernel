// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "badgeros-abi/errno.h"
#include "sched/waitlist.h"
#include "time.h"

#include <stdint.h>



// A counting semaphore.
typedef struct {
    waitlist_t       waitlist;
    _Atomic uint32_t counter;
} sem_t;

#define SEM_T_INIT ((sem_t){})

// Post once to the semaphore.
void    sem_post(sem_t *sem);
// Await one post from the semaphore.
// May fail with -EINTR if signalled.
errno_t sem_wait(sem_t *sem);
// Await one post from the semaphore.
// May fail with -EINTR if signalled.
errno_t sem_timed_wait(sem_t *sem, timestamp_us_t timeout);
