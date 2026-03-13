
// SPDX-FileCopyrightText: 2024-2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "hash.h"
#include "list.h"



// Create an empty hash map with C-string keys.
#define STR_MAP_EMPTY  ((map_t){NULL, 0, 0, &str_map_vtable})
// Create an empty hash map with pointer keys.
// Whatever is being pointer to is expected to live at least as long as the map.
#define PTR_MAP_EMPTY  ((map_t){NULL, 0, 0, &ptr_map_vtable})

// Iterate over all entries in the map.
#define map_foreach(varname, map)                                                                                      \
    for (map_ent_t const *varname = map_next(map, NULL); varname; varname = map_next(map, varname))



// Hash map.
typedef struct map        map_t;
// Hash map entry.
typedef struct map_ent    map_ent_t;
// Hash map vtable.
typedef struct map_vtable map_vtable_t;



// Hash map.
struct map {
    // Hash buckets; array of linked list of map_ent_t.
    dlist_t            *buckets;
    // Current number of buckets.
    size_t              buckets_len;
    // Current number of elements.
    size_t              len;
    // Map vtable.
    map_vtable_t const *vtable;
};

// Hash map entry.
struct map_ent {
    // Linked list node.
    dlist_node_t node;
    // Hash of key.
    uint32_t     hash;
    // Key.
    char        *key;
    // Value.
    void        *value;
};

// Hash map vtable.
struct map_vtable {
    // Key hashing function.
    uint32_t (*key_hash)(void const *);
    // Key comparison function; returns 0 if equal.
    int (*key_cmp)(void const *, void const *);
    // Key duplication function.
    void *(*key_dup)(void const *);
    // Key deletion function.
    void (*key_del)(void *);
};

// Vtable for string maps.
extern map_vtable_t const str_map_vtable;
// Vtable for pointer maps.
extern map_vtable_t const ptr_map_vtable;



// Remove all entries from a map.
void             map_clear(map_t *map);
// Get an item from the map.
void            *map_get(map_t const *map, void const *key) __attribute__((pure));
// Insert an item into the map.
bool             map_set(map_t *map, void const *key, void const *value);
// Remove an item from the map.
bool             map_remove(map_t *map, void const *key);
// Get next item in the map (or first if `ent` is NULL).
map_ent_t const *map_next(map_t const *map, map_ent_t const *ent);
