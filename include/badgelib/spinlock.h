
// SPDX-License-Identifier: MIT

#pragma once

#include <stdatomic.h>



typedef atomic_int spinlock_t;

#define SPINLOCK_T_INIT        0
#define SPINLOCK_T_INIT_SHARED 1



// Take the spinlock exclusively.
void spinlock_take(spinlock_t *lock);
// Release the spinlock exclusively.
void spinlock_release(spinlock_t *lock);
// Take the spinlock shared.
void spinlock_take_shared(spinlock_t *lock);
// Release the spinlock shared.
void spinlock_release_shared(spinlock_t *lock);
