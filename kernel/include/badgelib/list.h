
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// A node of a doubly linked list structure. It contains no data, use
// `field_parent_ptr` from `meta.h` to obtain the containing structure.
typedef struct dlist_node_t {
    // Pointer to the next item in the linked list.
    struct dlist_node_t *next;
    // Pointer to the previous item in the linked list.
    struct dlist_node_t *previous;
} dlist_node_t;

// A doubly linekd list.
typedef struct dlist_t {
    // Current number of elements in the list.
    size_t               len;
    // Pointer to the first node in the list or NULL if the list is empty.
    struct dlist_node_t *head;
    // Pointer to the last node in the list or NULL if the list is empty.
    struct dlist_node_t *tail;
} dlist_t;

// Initializer value for an empty list. Convenience macro for
// zero-initialization.
#define DLIST_EMPTY ((dlist_t){.len = 0, .head = NULL, .tail = NULL})

// Initializer value for a list node. Convenience macro for zero-initialization.
#define DLIST_NODE_EMPTY ((dlist_node_t){.next = NULL, .previous = NULL})

#ifndef container_of
#define container_of(ptr, type, member)                                                                                \
    ({                                                                                                                 \
        _Static_assert(sizeof(((type *)0)->member) == sizeof(*(ptr)), "Invalid member size");                          \
        (type *)((size_t)ptr - offsetof(type, member));                                                                \
    })
#endif

// Generate a foreach loop for a dlist.
#define dlist_foreach(type, varname, nodename, list)                                                                   \
    for (type *varname = container_of((list)->head, type, nodename); &varname->nodename;                               \
         varname       = container_of(varname->nodename.next, type, nodename))

// Generate a foreach loop for a dlist where the node name is `node`.
#define dlist_foreach_node(type, varname, list) dlist_foreach(type, varname, node, list)

// Concatenates the elements from dlist `back` on dlist `front`, clearing `back` in the process.
// Both lists must be non-NULL.
void dlist_concat(dlist_t *front, dlist_t *back);

// Appends `node` after the `tail` of the `list`.
// `node` must not be in `list` already.
// Both `list` and `node` must be non-NULL.
void dlist_append(dlist_t *list, dlist_node_t *node);

// Prepends `node` before the `head` of the `list`.
// `node` must not be in `list` already.
// Both `list` and `node` must be non-NULL.
void dlist_prepend(dlist_t *list, dlist_node_t *node);

// Removes the `head` of the given `list`. Will return NULL if the list was empty.
// `list` must be non-NULL.
dlist_node_t *dlist_pop_front(dlist_t *list);

// Removes the `tail` of the given `list`. Will return NULL if the list was empty.
// `list` must be non-NULL.
dlist_node_t *dlist_pop_back(dlist_t *list);

// Checks if `list` contains the given `node`.
// Both `list` and `node` must be non-NULL.
bool dlist_contains(dlist_t const *list, dlist_node_t const *node);

// Removes `node` from `list`. `node` must be either an empty (non-inserted) node or must be contained in `list`.
// Both `list` and `node` must be non-NULL.
void dlist_remove(dlist_t *list, dlist_node_t *node);
