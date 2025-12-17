
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "radixtree.h"

#include "assertions.h"
#include "badgeros-abi/errno.h"
#include "cpu/interrupt.h"
#include "rawprint.h"
#include "rcu.h"
#include "spinlock.h"
#include "time.h"

#include <stdatomic.h>
#include <stdint.h>

#include <malloc.h>



// Initiate a stack or dynamically allocated radix tree.
void rtree_init(rtree_t *tree) {
    *tree = (rtree_t)RTREE_T_INIT;
    atomic_thread_fence(memory_order_release);
}

// Get a value from the radix tree.
void *rtree_get(rtree_t *tree, uint64_t key) {
    bool ie = irq_disable();
    rcu_crit_enter();
    rtree_node_t *cur = atomic_load_explicit(&tree->root, memory_order_acquire);
    if (!cur || (cur->height < 64 && (key >> cur->height) >= RTREE_ENTS_PER_NODE)) {
        // No content or the key is not covered by the root node.
        rcu_crit_exit();
        irq_enable_if(ie);
        return NULL;
    }

    while (cur) {
        size_t index = (key >> cur->height) % RTREE_ENTS_PER_NODE;
        if (cur->height == 0) {
            rcu_crit_exit();
            irq_enable_if(ie);
            return atomic_load_explicit(&cur->data[index], memory_order_acquire);
        } else {
            cur = atomic_load_explicit(&cur->children[index], memory_order_acquire);
        }
    }

    rcu_crit_exit();
    irq_enable_if(ie);
    return NULL;
}

// Garbage-collect nodes along the path to `key`.
static void rtree_gc_key(rtree_t *tree, uint64_t key) {
    return;
    size_t        to_free_len = 0;
    rtree_node_t *to_free[63 / RTREE_BITS_PER_NODE + 1];
    bool          ie = irq_disable();
    spinlock_take(&tree->lock);

    // Walk down the tree to the target node as far as possible, locking on the way.
    rtree_node_t *node = tree->root;
    if (!node || (key >> node->height) >= RTREE_ENTS_PER_NODE) {
        spinlock_release(&tree->lock);
        irq_enable_if(ie);
        return;
    }
    while (1) {
        spinlock_take(&node->lock);
        if (node->height == 0) {
            break;
        }
        size_t        index = (key >> node->height) % RTREE_ENTS_PER_NODE;
        rtree_node_t *child = node->children[index];
        if (child == NULL) {
            break;
        }
        node = child;
    }

    // Walk back up to the highest freeable node and atomically unlink it.
    while (node && node->occupancy == 0) {
        to_free[to_free_len++] = node;
        node                   = node->parent;
        if (node) {
            node->occupancy--;
            size_t index = (key >> node->height) % RTREE_ENTS_PER_NODE;
            atomic_store_explicit(&node->children[index], NULL, memory_order_release);
        } else {
            atomic_store_explicit(&tree->root, NULL, memory_order_release);
        }
    }

    // Unlock the parent node and upwards.
    // The nodes to be freed won't be seen by other writers so they don't need to be unlocked.
    while (node) {
        spinlock_release(&node->lock);
        node = node->parent;
    }

    spinlock_release(&tree->lock);
    irq_enable_if(ie);

    // After an RCU grace period, free the nodes.
    if (to_free_len) {
        rcu_sync();
        for (size_t i = 0; i < to_free_len; i++) {
            free(to_free[i]);
        }
    }
}

// Create a new span of the radix tree from `max_height` downto 0, along the path for `key`.
// If successful, `value` will be stored at `key` in this subtree.
static rtree_node_t *rtree_create_subtree(uint64_t key, void *value, uint8_t max_height) {
    assert_dev_drop(max_height % RTREE_BITS_PER_NODE == 0);
    rtree_node_t *node = calloc(1, sizeof(rtree_node_t));
    if (!node) {
        return NULL;
    }
    node->height = max_height;
    if (max_height == 0) {
        // Leaf node: store value.
        size_t index = key % RTREE_ENTS_PER_NODE;
        atomic_store_explicit(&node->data[index], value, memory_order_release);
        node->occupancy = value != NULL;
    } else {
        // Internal node: create child subtree.
        size_t        index   = (key >> max_height) % RTREE_ENTS_PER_NODE;
        rtree_node_t *child   = rtree_create_subtree(key, value, max_height - RTREE_BITS_PER_NODE);
        node->children[index] = child;
        if (child) {
            child->parent   = node;
            node->occupancy = 1;
        } else {
            free(node);
            return NULL;
        }
    }
    return node;
}

// Implementation of `rtree_set` and `rtree_cmpxchg`.
static errno_ptr_t rtree_xchg(rtree_t *tree, uint64_t key, void *old_value, void *new_value, bool is_cmpxchg) {
    errno_ptr_t res;
    bool        ie = irq_disable();

    spinlock_t *cur_lock = &tree->lock;
    spinlock_take(cur_lock);
    rtree_node_t *cur = tree->root;

    if (new_value == NULL && (cur == NULL || (cur->height < 64 && (key >> cur->height) >= RTREE_ENTS_PER_NODE))) {
        // Setting NULL into an out-of-tree location.
        res.errno = 0;
        res.ptr   = NULL;

    } else if (cur == NULL) {
        // Make new tree for key.
        res.errno = 0;
        res.ptr   = NULL;
        if (!is_cmpxchg || old_value == NULL) {
            uint8_t height     = 64 - __builtin_clzll(key);
            height             = height / RTREE_BITS_PER_NODE * RTREE_BITS_PER_NODE;
            rtree_node_t *root = rtree_create_subtree(key, new_value, height);
            tree->root         = root;
            if (!root) {
                res.errno = -ENOMEM;
            }
        }

    } else {
        while (cur->height < 64 && (key >> cur->height) >= RTREE_ENTS_PER_NODE) {
            // Make tree tall enough to fit the key.
            rtree_node_t *root = calloc(1, sizeof(rtree_node_t));
            if (!root) {
                spinlock_release(cur_lock);
                irq_enable_if(ie);
                res.errno = -ENOMEM;
                return res;
            }
            root->occupancy   = 1;
            root->height      = cur->height + RTREE_BITS_PER_NODE;
            root->children[0] = cur;
            cur->parent       = root;
            tree->root        = root;
            cur               = root;
        }

        while (1) {
            // Walk tree then update.
            size_t index = (key >> cur->height) % RTREE_ENTS_PER_NODE;
            if (cur->height == 0) {
                // Exchange in leaf node.
                bool did_update;
                if (is_cmpxchg) {
                    did_update = atomic_compare_exchange_strong_explicit(
                        &cur->data[index],
                        &old_value,
                        new_value,
                        memory_order_acq_rel,
                        memory_order_acquire
                    );
                    res.ptr = old_value;
                } else {
                    res.ptr    = atomic_exchange_explicit(&cur->data[index], new_value, memory_order_acq_rel);
                    did_update = true;
                }
                if (did_update && res.ptr && !new_value) {
                    cur->occupancy--;
                } else if (did_update && !res.ptr && new_value) {
                    cur->occupancy++;
                }
                break;

            } else {
                spinlock_t *next_lock = &cur->lock;
                spinlock_take(next_lock);
                rtree_node_t *next = cur->children[index];
                if (next) {
                    // Walk further down the tree.
                    spinlock_release(cur_lock);
                    cur      = next;
                    cur_lock = next_lock;

                } else {
                    if (old_value != NULL && is_cmpxchg) {
                        res.errno = 0;
                        res.ptr   = NULL;
                        break;
                    }

                    // Create new leaf node.
                    next                 = rtree_create_subtree(key, new_value, cur->height - RTREE_BITS_PER_NODE);
                    next->parent         = cur;
                    cur->children[index] = next;
                    cur->occupancy++;
                    spinlock_release(next_lock);
                    res.errno = next ? 0 : -ENOMEM;
                    res.ptr   = NULL;
                    break;
                }
            }
        }
    }

    spinlock_release(cur_lock);
    irq_enable_if(ie);

    if (!new_value && (!is_cmpxchg || (res.errno >= 0 && res.ptr == old_value))) {
        rtree_gc_key(tree, key);
    }
    return res;
}

// Insert a value into the radix tree.
errno_ptr_t rtree_set(rtree_t *tree, uint64_t key, void *value) {
    return rtree_xchg(tree, key, NULL, value, false);
}

// Optimization of `rtree_cmpxchg` where `old_value` is not NULL.
static void *rtree_cmpxchg_nonzero(rtree_t *tree, uint64_t key, void *old_value, void *new_value) {
    bool ie = irq_disable();
    rcu_crit_enter();
    rtree_node_t *cur = atomic_load_explicit(&tree->root, memory_order_acquire);
    if (!cur || (cur->height < 64 && (key >> cur->height) >= RTREE_BITS_PER_NODE)) {
        // No content or the key is not covered by the root node.
        rcu_crit_exit();
        irq_enable_if(ie);
        return NULL;
    }

    while (cur) {
        size_t index = (key >> cur->height) % RTREE_ENTS_PER_NODE;
        if (cur->height == 0) {
            rcu_crit_exit();
            irq_enable_if(ie);
            atomic_compare_exchange_strong_explicit(
                &cur->data[index],
                &old_value,
                new_value,
                memory_order_acq_rel,
                memory_order_acquire
            );
            return old_value;
        } else {
            cur = atomic_load_explicit(&cur->children[index], memory_order_acquire);
        }
    }

    rcu_crit_exit();
    irq_enable_if(ie);
    return NULL;
}

// Compare-exchange a value in radix tree.
// Returns the old value regardless of success.
errno_ptr_t rtree_cmpxchg(rtree_t *tree, uint64_t key, void *old_value, void *new_value) {
    if (old_value) {
        return rtree_xchg(tree, key, old_value, new_value, true);
    } else {
        return (errno_ptr_t){rtree_cmpxchg_nonzero(tree, key, old_value, new_value), 0};
    }
}

// Recursive implementation of locking for `rtree_clear`.
static void rtree_clear_lock(rtree_node_t *node) {
    spinlock_take(&node->lock);
    if (node->height) {
        for (size_t i = 0; i < RTREE_ENTS_PER_NODE; i++) {
            rtree_clear_lock(node->children[i]);
        }
    }
}

// Recursive implementation of freeing for `rtree_clear`.
static void rtree_clear_free(rtree_node_t *node) {
    if (node->height) {
        for (size_t i = 0; i < RTREE_ENTS_PER_NODE; i++) {
            rtree_clear_free(node->children[i]);
        }
    }
    free(node);
}

// Clear the radix tree.
void rtree_clear(rtree_t *tree) {
    bool ie = irq_disable();
    spinlock_take(&tree->lock);
    rtree_node_t *root = atomic_exchange_explicit(&tree->root, NULL, memory_order_acq_rel);
    if (root) {
        // Lock all so we wait for any pending writes to complete.
        rtree_clear_lock(root);
    }
    irq_enable_if(ie);

    if (root) {
        // After an RCU grace period, free the nodes.
        rcu_sync();
        rtree_clear_free(root);
    }
}



// Implementation of `rtree_first` and `rtree_next`.
static rtree_iter_t rtree_iter_impl(rtree_t *tree, rtree_iter_t cur) {
    rtree_node_t const *const root = atomic_load_explicit(&tree->root, memory_order_acquire);
    uint8_t  max_key_bits          = root->height + RTREE_BITS_PER_NODE < 64 ? root->height + RTREE_BITS_PER_NODE : 64;
    uint64_t max_key               = (1llu << max_key_bits) - 1;
    while (cur.node) {
        size_t index = (cur.key >> cur.node->height) % RTREE_ENTS_PER_NODE;
        if (cur.node->height == 0) {
            void *const value = atomic_load_explicit(&cur.node->data[index], memory_order_acquire);
            if (value) {
                cur.value = value;
                return cur;
            } else {
                cur.key++;
                if (index == RTREE_ENTS_PER_NODE - 1) {
                    cur.node = cur.node->parent;
                }
            }
        } else {
            rtree_node_t const *const next = atomic_load_explicit(&cur.node->children[index], memory_order_acquire);
            if (next) {
                cur.node = next;
            } else {
                cur.key += 1llu << cur.node->height;
                if (cur.key == max_key) {
                    break;
                } else if (index == RTREE_ENTS_PER_NODE - 1) {
                    cur.node = cur.node->parent;
                }
            }
        }
    }
    return (rtree_iter_t){NULL, 0, NULL};
}

// Get the first entry in the radix tree.
rtree_iter_t rtree_first(rtree_t *tree, uint64_t min_key) {
    rcu_crit_assert();
    rtree_node_t const *const root = atomic_load_explicit(&tree->root, memory_order_acquire);
    if (root->height < 64 && min_key >> root->height >= RTREE_ENTS_PER_NODE) {
        return (rtree_iter_t){NULL, 0, NULL};
    }
    return rtree_iter_impl(tree, (rtree_iter_t){root, min_key, NULL});
}

// Get the next entry in the radix tree.
rtree_iter_t rtree_next(rtree_t *tree, rtree_iter_t cur) {
    rcu_crit_assert();
    while ((cur.key >> cur.node->height) % RTREE_ENTS_PER_NODE == RTREE_ENTS_PER_NODE - 1) {
        cur.node = cur.node->parent;
        if (!cur.node) {
            return (rtree_iter_t){NULL, 0, NULL};
        }
    }
    cur.key++;
    return rtree_iter_impl(tree, cur);
}

static void print_ptr(int indent, void *value) {
    rawprint(" val 0x");
    rawprinthex((size_t)value, sizeof(size_t) * 2);
    rawprint("\n");
}

static void rtree_dump_impl(rtree_node_t *node, void (*printer)(int indent, void *value), int indent) {
    rawprint(": ");
    rawprintdec(node->occupancy, 0);
    rawprint("\n");
    for (size_t i = 0; i < RTREE_ENTS_PER_NODE; i++) {
        if (!node->children[i]) {
            continue;
        }
        for (int x = 0; x < indent; x++) {
            rawprint("  ");
        }
        rawprint("[");
        rawprintdec((int64_t)i, 2);
        rawprint(" @ ");
        rawprintdec(node->height, 0);
        rawprint("]");
        if (node->height == 0) {
            rawprint(":");
            printer(indent + 1, node->data[i]);
        } else {
            rtree_dump_impl(node->children[i], printer, indent + 1);
        }
    }
}

// Debug-print a radix tree.
void rtree_dump(rtree_t *tree, void (*printer)(int indent, void *value)) {
    if (tree->root == NULL) {
        rawprint("<empty>\n");
    } else {
        rawprint("[root]");
        rtree_dump_impl(tree->root, printer ?: print_ptr, 1);
    }
}
