
// SPDX-License-Identifier: MIT

#pragma once

// Privilege level bitmask.
#define PRIV_LEVEL_MASK 3llu
// Privilege level in which the kernel runs.
#define PRIV_KERNEL     0llu
// Privilege level in which user code runs.
#define PRIV_USER       3llu
