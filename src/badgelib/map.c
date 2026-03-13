
// SPDX-FileCopyrightText: 2024-2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "map.h"

#include "badge_strings.h"

#include <malloc.h>



// Vtable for string maps.
map_vtable_t const str_map_vtable = {
    .key_cmp  = (int (*)(void const *, void const *))cstr_compare,
    .key_del  = free,
    .key_dup  = (void *(*)(void const *))cstr_duplicate,
    .key_hash = (uint32_t (*)(void const *))hash_cstr,
};

// Vtable for pointer maps.
map_vtable_t const ptr_map_vtable = {
    .key_hash = hash_ptr,
    .key_cmp  = cmp_ptr,
    .key_dup  = dup_nop,
    .key_del  = del_nop,
};

// Threshold of max items per bucket.
// Note: The number is estimated by total size as opposed to actually counting.
#define MAP_MAX_BUCKET_SIZE 4



// Change the amount of buckets that a map has.
bool map_resize(map_t *map, size_t new_buckets_len) {
    if (new_buckets_len == map->buckets_len) {
        return true;
    }

    // Try to allocate memory for the new buckets.
    dlist_t *new_buckets = calloc(new_buckets_len, sizeof(dlist_t));
    if (!new_buckets) {
        return false;
    }

    // Sort entries into the new correct buckets.
    for (size_t i = 0; i < map->buckets_len; i++) {
        map_ent_t *ent;
        while ((ent = container_of(dlist_pop_front(&map->buckets[i]), map_ent_t, node))) {
            dlist_append(&new_buckets[ent->hash & (new_buckets_len - 1)], &ent->node);
        }
    }

    // Update the map's pointers.
    free(map->buckets);
    map->buckets     = new_buckets;
    map->buckets_len = new_buckets_len;
    return true;
}

// Try to resize the map according to the current occupancy (but don't fail if OOM).
void map_auto_resize(map_t *map) {
    if (map->len == 0) {
        free(map->buckets);
        map->buckets     = NULL;
        map->buckets_len = 0;
        return;
    }

    // Calculate how many buckets this map would need.
    size_t min_buckets = (map->len + MAP_MAX_BUCKET_SIZE - 1) / MAP_MAX_BUCKET_SIZE;
    // Round up to the next power of 2.
    if (min_buckets & (min_buckets - 1)) {
        min_buckets = 1lu << (1 + 8 * sizeof(size_t) - __builtin_clzl(min_buckets));
    }

    map_resize(map, min_buckets);
}



// Remove all entries from a map.
void map_clear(map_t *map) {
    for (size_t i = 0; i < map->buckets_len; i++) {
        dlist_node_t *node = dlist_pop_front(&map->buckets[i]);
        while (node) {
            map_ent_t *ent = (map_ent_t *)node;
            map->vtable->key_del(ent->key);
            free(ent);
            node = dlist_pop_front(&map->buckets[i]);
        }
    }
    free(map->buckets);
    map->buckets     = NULL;
    map->buckets_len = 0;
    map->len         = 0;
}

// Get an item from the map.
void *map_get(map_t const *map, void const *key) {
    if (!map->len) {
        return NULL;
    }

    // Figure out which bucket the key is in.
    uint32_t hash   = map->vtable->key_hash(key);
    size_t   bucket = hash & (map->buckets_len - 1);

    // Walk the list of items in this bucket.
    dlist_node_t *node = map->buckets[bucket].head;
    while (node) {
        map_ent_t *ent = (map_ent_t *)node;
        // Both hash and string compare must be equal.
        if (ent->hash == hash && !map->vtable->key_cmp(ent->key, key)) {
            return ent->value;
        }
        // Go to the next item in the bucket.
        node = node->next;
    }

    // The bucket did not contain the key.
    return NULL;
}

// Insert an item into the map.
bool map_set(map_t *map, void const *key, void const *value) {
    if (!map->buckets_len) {
        if (!map_resize(map, 2)) {
            return false;
        }
    }

    // Figure out which bucket the key is in.
    uint32_t hash   = map->vtable->key_hash(key);
    size_t   bucket = hash & (map->buckets_len - 1);

    // Walk the list of items in this bucket.
    dlist_node_t *node = map->buckets[bucket].head;
    while (node) {
        map_ent_t *ent = (map_ent_t *)node;
        // Both hash and string compare must be equal.
        if (ent->hash == hash && !map->vtable->key_cmp(ent->key, key)) {
            if (value) {
                // Overwrite existing value.
                ent->value = (void *)value;
            } else {
                // Remove existing value.
                dlist_remove(&map->buckets[bucket], node);
                map->vtable->key_del(ent->key);
                free(ent);
                map->len--;
                map_auto_resize(map);
            }
            // Successfully set the item.
            return true;
        }
        node = node->next;
    }

    if (!value) {
        // No item to be removed.
        return true;
    }

    // Allocate a new item.
    map_ent_t *ent = malloc(sizeof(map_ent_t));
    if (!ent) {
        return false;
    }
    ent->node  = DLIST_NODE_EMPTY;
    ent->key   = map->vtable->key_dup(key);
    ent->value = (void *)value;
    ent->hash  = hash;

    // Add the new item to the bucket.
    dlist_append(&map->buckets[bucket], &ent->node);
    map->len++;
    map_auto_resize(map);
    return true;
}

// Remove an item from the map.
bool map_remove(map_t *map, void const *key) {
    return map_set(map, key, NULL);
}

// Get next item in the map (or first if `ent` is NULL).
map_ent_t const *map_next(map_t const *map, map_ent_t const *ent) {
    if (!map->len) {
        return NULL;
    }
    size_t bucket;
    if (!ent) {
        bucket = 0;
    } else if (ent->node.next) {
        return (map_ent_t const *)ent->node.next;
    } else {
        bucket = (ent->hash & (map->buckets_len - 1)) + 1;
    }
    while (bucket < map->buckets_len) {
        if (map->buckets[bucket].head) {
            return (map_ent_t const *)map->buckets[bucket].head;
        }
        bucket++;
    }
    return NULL;
}
