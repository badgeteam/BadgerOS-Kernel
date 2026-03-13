
// SPDX-License-Identifier: MIT

#pragma once

#include "list.h"
#include "scheduler/scheduler.h"
#include "spinlock.h"



// Thread waitilig list entry.
typedef struct {
    // Linked list node.
    dlist_node_t node;
    // Whether this entry is still in the list.
    bool         in_list;
    // Thread to resume.
    tid_t        thread;
    // Blocking ticket to resume.
    uint64_t     ticket;
} waitlist_ent_t;

// Thread waiting list object.
typedef struct {
    // Waiting list spinlock.
    spinlock_t lock;
    // List of blocked threads.
    dlist_t    list;
} waitlist_t;

// Create a new (empty) waiting list.
#define WAITLIST_T_INIT {SPINLOCK_T_INIT, DLIST_EMPTY}



// Block on a waiting list, or until a timeout is reached.
// Runs `double_check(cookie)` and unblocks if false to prevent race conditions causing deadlocks.
void waitlist_block(waitlist_t *list, timestamp_us_t timeout, bool (*double_check)(void *), void *cookie);
// Resume the first thread from the waiting list, if there is one.
void waitlist_notify(waitlist_t *list);
