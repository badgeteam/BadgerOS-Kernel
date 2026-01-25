// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "errno.h"



// A single-byte fallible store.
errno_t fallible_store_u8(uint8_t *addr, uint8_t value);

// A single-byte fallible load.
errno_t fallible_load_u8(uint8_t const *addr);

// Raw `memcpy` from kernel to user memory.
// Catches faults on `dest` but will panic for faults on `src`.
errno_t copy_to_user(void *dest, void const *src, size_t len);

// Raw `memcpy` from user to kernel memory.
// Catches faults on `src` but will panic for faults on `dest`.
errno_t copy_from_user(void *dest, void const *src, size_t len);
