
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// Get the hash of a C-string.
uint32_t hash_cstr(char const *str);
// Get the hash of a byte vector.
uint32_t hash_bytes(void const *data, size_t len);
// Get the hash of a pointer.
uint32_t hash_ptr(void const *ptr);

// Compare a pointer for equality.
int cmp_ptr(void const *a, void const *b);

// No-op duplicate function; simply returns argument.
void *dup_nop(void const *a);

// No-op delete function; does absolutely nothing.
void del_nop(void *a);
