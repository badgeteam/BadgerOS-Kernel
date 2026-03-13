
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "hash.h"



// Get the hash of a C-string.
uint32_t hash_cstr(char const *str) {
    uint32_t hash = 5381;

    while (*str) {
        hash = hash * 33 + *str;
        str++;
    }

    return hash;
}

// Get the hash of a byte vector.
uint32_t hash_bytes(void const *data, size_t len) {
    uint8_t const *str  = data;
    uint32_t       hash = 5381;

    for (size_t i = 0; i < len; i++) {
        hash = hash * 33 + str[i];
    }

    return hash;
}

// Get the hash of a pointer.
uint32_t hash_ptr(void const *ptr) {
#if __SIZE_MAX__ == __UINT64_MAX__
    uint32_t val = (size_t)ptr ^ ((size_t)ptr >> 32);
#else
    uint32_t val = (size_t)ptr;
#endif
    return val * 2654435761;
}



// Compare a pointer for equality.
int cmp_ptr(void const *a, void const *b) {
    return a != b;
}



// No-op duplicate function; simply returns argument.
void *dup_nop(void const *a) {
    return (void *)a;
}



// No-op delete function; does absolutely nothing.
void del_nop(void *) {
}
