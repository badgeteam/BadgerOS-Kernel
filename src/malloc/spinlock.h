// SPDX-License-Identifier: MIT

#pragma once

#include "compiler.h"

#include <stdatomic.h>
#include <stdbool.h>

#define SPIN_LOCK_LOCK(x)                                                                                              \
    do {                                                                                                               \
        int __spin_backoff = 1;                                                                                        \
        while (atomic_flag_test_and_set_explicit(&x, memory_order_acquire)) {                                          \
            for (int __spin_i = 0; __spin_i < __spin_backoff; ++__spin_i) intr_pause();                                \
            if (__spin_backoff < 16) /* limit backoff to 16 pauses */                                                  \
                __spin_backoff <<= 1;                                                                                  \
        }                                                                                                              \
    } while (0)

#define SPIN_LOCK_TRY_LOCK(x) !atomic_flag_test_and_set_explicit(&x, memory_order_acquire)
#define SPIN_LOCK_UNLOCK(x)   atomic_flag_clear_explicit(&x, memory_order_release)
#define DELAY(x)                                                                                                       \
    do {                                                                                                               \
        for (size_t i = 0; i < x; ++i) {                                                                               \
            intr_pause();                                                                                              \
        }                                                                                                              \
    } while (0)
