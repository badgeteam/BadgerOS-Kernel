// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "badgeros-abi/errno.h"
#include "list.h"

#include <stddef.h>



// Condition evaluator function for `waitlist_block`.
typedef bool (*waitlist_condfn_t)(void *cookie);

// Helper struct used to construct types that block threads.
typedef struct {
    uint32_t list_lock;
    dlist_t  list;
} waitlist_t;

#define WAITLIST_T_INIT ((waitlist_t){0, DLIST_EMPTY})

// Block on this list if a condition is met.
// May spuriously return early, or return -EINTR if the thread was signalled.
errno_t waitlist_block(waitlist_t *list, waitlist_condfn_t cond, void *cookie);
/// Notify at least one thread on this list.
void    waitlist_notify(waitlist_t *list);
/// Notify all threads on this list.
void    waitlist_notify_all(waitlist_t *list);
