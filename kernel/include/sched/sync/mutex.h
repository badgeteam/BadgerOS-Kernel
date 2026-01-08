// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "errno.h"
#include "sched/waitlist.h"
#include "time.h"

#include <stdint.h>



// Raw mutually-exclusive resource access guard.
typedef struct {
    waitlist_t       waitlist;
    _Atomic uint32_t shares;
} mutex_t;

#define MUTEX_T_INIT ((mutex_t){})

errno_t mutex_lock(mutex_t *mutex);
errno_t mutex_lock_shared(mutex_t *mutex);
errno_t mutex_timed_lock(mutex_t *mutex, timestamp_us_t timeout);
errno_t mutex_timed_lock_shared(mutex_t *mutex, timestamp_us_t timeout);
void    mutex_unlock(mutex_t *mutex);
void    mutex_unlock_shared(mutex_t *mutex);
