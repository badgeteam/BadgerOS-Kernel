
// SPDX-License-Identifier: MIT

#include "mutex.h"

#include "assertions.h"
#include "scheduler/waitlist.h"
#include "time.h"

#include <stdatomic.h>

// Magic value for exclusive locking.
#define EXCLUSIVE_MAGIC ((int)__INT_MAX__ / 4)



// Recommended way to create a mutex at run-time.
void mutex_init(mutex_t *mutex, bool shared) {
    *mutex = ((mutex_t){shared, 0, WAITLIST_T_INIT});
    atomic_thread_fence(memory_order_release);
}

// Clean up the mutex.
void mutex_destroy(mutex_t *mutex) {
    // The mutex must always be completely unlocked to guarantee no threads are waiting on it.
    assert_always(mutex->shares == 0);
}



bool mutex_block_double_check(void *cookie) {
    struct {
        mutex_t *mutex;
        int      double_check_value;
    } *data = cookie;
    return atomic_load(&data->mutex->shares) == data->double_check_value;
}

// Mutex blocking implementation.
// If the mutex's value changes from `double_check_value`,
// the block is instantly cancelled to prevent race conditions locking up the thread.
static void mutex_block(mutex_t *mutex, timestamp_us_t timeout, int double_check_value) {
    struct {
        mutex_t *mutex;
        int      double_check_value;
    } data = {
        mutex,
        double_check_value,
    };
    waitlist_block(&mutex->waiting_list, timeout, mutex_block_double_check, &data);
}

// Notify the first waiting thread of the mutex being released.
static void mutex_notify(mutex_t *mutex) {
    waitlist_notify(&mutex->waiting_list);
}

// Atomically await the expected value and swap in the new value.
static inline bool
    await_swap_atomic_int(mutex_t *mutex, timestamp_us_t timeout, int expected, int new_value, memory_order order) {
    int loops = MUTEX_FAST_LOOPS;
    do {
        int old_value = expected;
        if (atomic_compare_exchange_weak_explicit(&mutex->shares, &old_value, new_value, order, memory_order_relaxed)) {
            return true;
        } else if (loops) {
            loops--;
        } else {
            mutex_block(mutex, timeout, old_value);
        }
    } while (time_us() < timeout);
    return false;
}

// Atomically check the value does not exceed a threshold and add 1.
static inline bool thresh_add_atomic_int(mutex_t *mutex, timestamp_us_t timeout, int threshold, memory_order order) {
    int old_value = atomic_load(&mutex->shares);
    int loops     = MUTEX_FAST_LOOPS;
    do {
        int new_value = old_value + 1;
        if (!(old_value >= threshold || new_value >= threshold) &&
            atomic_compare_exchange_weak_explicit(&mutex->shares, &old_value, new_value, order, memory_order_relaxed)) {
            return true;
        } else {
            if (loops) {
                loops--;
            } else {
                mutex_block(mutex, timeout, old_value);
            }
            old_value = atomic_load(&mutex->shares);
        }
    } while (time_us() < timeout);
    return false;
}

// Atomically check the value doesn't equal either illegal values and subtract 1.
static inline bool unequal_sub_atomic_int(mutex_t *mutex, int unequal0, int unequal1, memory_order order) {
    int old_value = atomic_load(&mutex->shares);
    while (1) {
        int new_value = old_value - 1;
        if (!(old_value == unequal0 || old_value == unequal1) &&
            atomic_compare_exchange_weak_explicit(&mutex->shares, &old_value, new_value, order, memory_order_relaxed)) {
            return true;
        } else {
            thread_yield();
        }
    }
}



// Try to acquire `mutex` within `timeout` microseconds.
// Returns true if the mutex was successully acquired.
bool mutex_acquire(mutex_t *mutex, timestamp_us_t timeout) {
    // Compute timeout.
    timestamp_us_t now = time_us();
    if (timeout < 0 || timeout - TIMESTAMP_US_MAX + now >= 0) {
        timeout = TIMESTAMP_US_MAX;
    } else {
        timeout += now;
    }
    // Await the shared portion to reach 0 and then lock.
    if (await_swap_atomic_int(mutex, timeout, 0, EXCLUSIVE_MAGIC, memory_order_acquire)) {
        // If that succeeds, the mutex was acquired.
        return true;
    } else {
        // Acquire failed.
        return false;
    }
}

// Release `mutex`, if it was initially acquired by this thread.
// Returns true if the mutex was successfully released.
bool mutex_release(mutex_t *mutex) {
    assert_dev_drop(atomic_load(&mutex->shares) >= EXCLUSIVE_MAGIC);
    if (await_swap_atomic_int(mutex, TIMESTAMP_US_MAX, EXCLUSIVE_MAGIC, 0, memory_order_release)) {
        // Successful release.
        mutex_notify(mutex);
        return true;
    } else {
        // Mutex was not taken exclusively.
        return false;
    }
}

// Try to acquire a share in `mutex` within `timeout` microseconds.
// Returns true if the share was successfully acquired.
bool mutex_acquire_shared(mutex_t *mutex, timestamp_us_t timeout) {
    if (!mutex->is_shared) {
        return false;
    }
    // Compute timeout.
    timestamp_us_t now = time_us();
    if (timeout < 0 || timeout - TIMESTAMP_US_MAX + now >= 0) {
        timeout = TIMESTAMP_US_MAX;
    } else {
        timeout += now;
    }
    // Take a share.
    if (thresh_add_atomic_int(mutex, timeout, EXCLUSIVE_MAGIC, memory_order_acquire)) {
        // If that succeeds, the mutex was successfully acquired.
        return true;
    } else {
        // If that fails, abort trying to lock.
        return false;
    }
}

// Release `mutex`, if it was initially acquired by this thread.
// Returns true if the mutex was successfully released.
bool mutex_release_shared(mutex_t *mutex) {
    assert_dev_drop(atomic_load(&mutex->shares) < EXCLUSIVE_MAGIC);
    if (!unequal_sub_atomic_int(mutex, 0, EXCLUSIVE_MAGIC, memory_order_release)) {
        // Prevent the counter from underflowing.
        return false;
    } else {
        // Successful release.
        mutex_notify(mutex);
        return true;
    }
}
