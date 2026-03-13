
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "attributes.h"
#include "hash.h"
#include "list.h"



// Create an empty hash set for C-strings.
#define STR_SET_EMPTY ((set_t){NULL, 0, 0, &str_set_vtable})
// Create an empty hash set for pointers.
// Whatever is being pointer to is expected to live at least as long as the set.
#define PTR_SET_EMPTY ((set_t){NULL, 0, 0, &ptr_set_vtable})



// Hash set.
typedef struct set        set_t;
// Hash set vtable.
typedef struct set_vtable set_vtable_t;
// Hash set entry.
typedef struct set_ent    set_ent_t;
// Option of a pointer that may be NULL.
typedef struct set_get    set_get_t;



// Hash set.
struct set {
    // Hash buckets; array of linked list of set_ent_t.
    dlist_t            *buckets;
    // Current number of buckets.
    size_t              buckets_len;
    // Current number of elements.
    size_t              len;
    // Set vtable.
    set_vtable_t const *vtable;
};

// Hash set vtable.
struct set_vtable {
    // Value hashing function.
    uint32_t (*val_hash)(void const *);
    // Value comparison function; returns 0 if equal.
    int (*val_cmp)(void const *, void const *);
    // Value duplication function; expected to abort if out of memory.
    void *(*val_dup)(void const *);
    // Value deletion function.
    void (*val_del)(void *);
};

// Hash set entry.
struct set_ent {
    // Linked list node.
    dlist_node_t node;
    // Hash of value.
    uint32_t     hash;
    // Value.
    void        *value;
};

// Option of a pointer that may be NULL.
struct set_get {
    bool  present;
    void *value;
};



// Iterate over all entries in the set.
#define set_foreach(type, varname, set)                                                                                \
    for (set_ent_t const *ent = set_next(set, NULL); ent; ent = set_next(set, ent))                                    \
        for (type *varname = ent->value; varname; varname = NULL)

// Vtable for string sets.
extern set_vtable_t const str_set_vtable;
// Vtable for pointer sets.
extern set_vtable_t const ptr_set_vtable;

// Remove all entries from a set.
void             set_clear(set_t *set);
// Get a pointer to an item from the set.
set_get_t        set_get(set_t const *set, void const *value) __attribute__((pure));
// Insert an item into the set.
bool             set_add(set_t *set, void const *value);
// Add all items from another set to this one.
size_t           set_addall(set_t *set, set_t const *other);
// Retain all items also in the other set.
// Returns how many items, if any, are removed.
size_t           set_intersect(set_t *set, set_t const *other);
// Retain all items that are in the array.
// Returns how many items, if any, are removed.
size_t           set_intersect_array(set_t *set, void const *const *values, size_t values_len);
// Remove an item from the set.
bool             set_remove(set_t *set, void const *value);
// Remove all items in another set from this one.
size_t           set_removeall(set_t *set, set_t const *other);
// Get next item in the set (or first if `ent` is NULL).
set_ent_t const *set_next(set_t const *set, set_ent_t const *ent);

#ifndef NDEBUG
// Print the pointer of all items in a set.
void set_dump(set_t const *set);
#endif

// Test if an item is in the set.
FORCEINLINE static inline bool set_contains(set_t const *set, void const *value) {
    return set_get(set, value).present;
}
