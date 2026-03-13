
// SPDX-License-Identifier: MIT

#include "spinlock.h"

#include "assertions.h"
#include "interrupt.h"

#define SPINLOCK_EXCL_MAGIC (-__INT_MAX__ - 1)



// Take the spinlock exclusively.
void spinlock_take(spinlock_t *lock) {
    assert_dev_drop(!irq_is_enabled());
    int cur = atomic_load_explicit(lock, memory_order_acquire);
    int next;
    do {
        cur  &= 1;
        next  = cur | SPINLOCK_EXCL_MAGIC;
    } while (!atomic_compare_exchange_weak_explicit(lock, &cur, next, memory_order_acquire, memory_order_relaxed));
}

// Release the spinlock exclusively.
void spinlock_release(spinlock_t *lock) {
    assert_dev_drop(!irq_is_enabled());
    int res = atomic_fetch_and_explicit(lock, 1, memory_order_release);
    assert_dev_drop(res < 0);
}

// Take the spinlock shared.
void spinlock_take_shared(spinlock_t *lock) {
    assert_dev_drop(!irq_is_enabled());
    assert_dev_drop(atomic_load_explicit(lock, memory_order_relaxed) & 1);
    int cur = atomic_load_explicit(lock, memory_order_acquire);
    int next;
    do {
        if (cur < 0) {
            cur = 1;
        }
        next = cur + 2;
    } while (!atomic_compare_exchange_weak_explicit(lock, &cur, next, memory_order_acquire, memory_order_relaxed));
}

// Release the spinlock shared.
void spinlock_release_shared(spinlock_t *lock) {
    assert_dev_drop(!irq_is_enabled());
    int res = atomic_fetch_sub_explicit(lock, 2, memory_order_release);
    assert_dev_drop(res >= 2);
}
