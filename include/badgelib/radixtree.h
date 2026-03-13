
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "badgeros-abi/errno.h"
#include "spinlock.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// How many bits per node are consumed in a radix tree.
#define RTREE_BITS_PER_NODE 6u
// How many entries are stored per node in a radix tree.
#define RTREE_ENTS_PER_NODE (1llu << RTREE_BITS_PER_NODE)

// A generic radix tree.
typedef struct rtree      rtree_t;
// A radix tree node.
typedef struct rtree_node rtree_node_t;
// A radix tree iterator.
typedef struct rtree_iter rtree_iter_t;

// A generic thread-safe radix tree.
// Inspired by the way Linux implements its xarray.
struct rtree {
    // Tree write spinlock.
    spinlock_t              lock;
    // Tree root node.
    _Atomic(rtree_node_t *) root;
};

// A radix tree node.
struct rtree_node {
    // Parent node, NULL for root node.
    rtree_node_t *parent;
    // Node write spinlock.
    spinlock_t    lock;
    // How far up the node is multiplied by RTREE_BITS_PER_NODE; 0 at leaf nodes.
    uint8_t       height;
    // How many child nodes are not empty.
    uint8_t       occupancy;
    union {
        // Child nodes.
        _Atomic(rtree_node_t *) children[RTREE_ENTS_PER_NODE];
        // Data pointers.
        _Atomic(void *)         data[RTREE_ENTS_PER_NODE];
    };
};

// A radix tree iterator.
struct rtree_iter {
    // Current node.
    rtree_node_t const *node;
    // Current key.
    uint64_t            key;
    // Current value.
    void               *value;
};

// Initialize an empty radix tree of pointers.
// Should only be used for static initializations; use `rtree_init()` for non-static.
#define RTREE_T_INIT {SPINLOCK_T_INIT, NULL}

// Iterate a radix tree by key and value.
#define rtree_foreach(iter_name, tree)                                                                                 \
    for (rtree_iter_t iter_name = rtree_first((tree), 0); iter_name.node; iter_name = rtree_next((tree), iter_name))



// Initiate a stack or dynamically allocated radix tree.
void        rtree_init(rtree_t *tree);
// Get a value from the radix tree.
void       *rtree_get(rtree_t *tree, uint64_t key);
// Insert a value into the radix tree, returning the old value.
errno_ptr_t rtree_set(rtree_t *tree, uint64_t key, void *value);
// Compare-exchange a value in radix tree.
// Writes the old value back if it was different.
errno_ptr_t rtree_cmpxchg(rtree_t *tree, uint64_t key, void *old_value, void *new_value);
// Clear the entire radix tree to NULL.
void        rtree_clear(rtree_t *tree);

// Get the first entry in the radix tree starting at a certain key.
rtree_iter_t rtree_first(rtree_t *tree, uint64_t min_key);
// Get the next entry in the radix tree.
rtree_iter_t rtree_next(rtree_t *tree, rtree_iter_t cur);

// Debug-print a radix tree.
void rtree_dump(rtree_t *tree, void (*printer)(int indent, void *value));
