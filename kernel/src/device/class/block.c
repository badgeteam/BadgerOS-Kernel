
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "device/class/block.h"

#include "assertions.h"
#include "badge_strings.h"
#include "cpu/interrupt.h"
#include "cpu/mmu.h"
#include "cpu/usercopy.h"
#include "log.h"
#include "radixtree.h"
#include "sched/sched.h"
#include "sched/sync/mutex.h"
#include "sched/sync/rcu.h"
#include "time.h"

#include <stdint.h>

#include <malloc.h>

// Batch size for syncing blocks.
#define BLK_SYNC_BATCH_SIZE 32

// Untag a block cache pointer.
#define BLK_UNTAG(ptr)      ((__typeof__(ptr))(((size_t)ptr) & ~(size_t)7))
// Tag a block cache pointer as locked.
#define BLK_TAG_LOCKED(ptr) ((__typeof__(ptr))((size_t)(ptr) | 1))
// Tag a block cache pointer as dirty.
#define BLK_TAG_DIRTY(ptr)  ((__typeof__(ptr))((size_t)(ptr) | 2))
// Tag a block cache pointer as being synced.
#define BLK_TAG_SYNC(ptr)   ((__typeof__(ptr))((size_t)(ptr) | 4))
// NULL pointer tagged as locked.
#define BLK_LOCKED_NULL     BLK_TAG_LOCKED(NULL)
// Whether a block cache pointer is tagged as locked.
#define BLK_IS_LOCKED(ptr)  (((size_t)(ptr) & 1) != 0)
// Whether a block cache pointer is tagged as dirty.
#define BLK_IS_DIRTY(ptr)   (((size_t)(ptr) & 2) != 0)
// Whether a block cache pointer is tagged as being synced.
#define BLK_IS_SYNC(ptr)    (((size_t)(ptr) & 4) != 0)



// [Implemented in Rust] Get the volume information for a particular drive.
get_volume_info_t get_volume_info(device_block_t *device);



// Called after a block device is activated.
errno_t device_block_activated(device_block_t *device) {
    if (!device->no_cache) {
        rtree_init(&device->cache);
    }
    device->volume_info_mtx = MUTEX_T_INIT;
    mem_set(&device->volume_info, 0, sizeof(device->volume_info));
    return 0;
}

// Called before a block device is removed.
void device_block_remove(device_block_t *device) {
}


// (Re-)scan partitions for this drive.
errno_t device_block_scan_parts(device_block_t *device) {
    mutex_lock(&device->volume_info_mtx);
    get_volume_info_t res = get_volume_info(device);

    if (res.errno >= 0) {
    }

    mutex_unlock(&device->volume_info_mtx);
    return res.errno;
}


// Write device blocks.
// The alignment for DMA is handled by this function.
errno_t device_block_write_blocks(device_block_t *device, uint64_t start, uint64_t count, void const *data) {
    return device_block_write_bytes(
        device,
        start << device->block_size_exp,
        count << device->block_size_exp,
        (void const __user *)data
    );
}

// Read device blocks.
// The alignment for DMA is handled by this function.
errno_t device_block_read_blocks(device_block_t *device, uint64_t start, uint64_t count, void *data) {
    return device_block_read_bytes(
        device,
        start << device->block_size_exp,
        count << device->block_size_exp,
        (void __user *)data
    );
}

// Erase blocks.
errno_t device_block_erase_blocks(device_block_t *device, uint64_t start, uint64_t count) {
    if (start + count < start || start + count > device->block_count) {
        logkf(
            LOG_WARN,
            "OOB block device access rejected; erasing 0x%{u64;x} @ 0x%{u64;x} on a 0x%{u64;x}*%{u64;d} device",
            start << device->block_size_exp,
            count << device->block_size_exp,
            device->block_count,
            1llu << device->block_size_exp
        );
        return -EINVAL;
    }

    logk(LOG_WARN, "TODO: device_block_erase_blocks");

    return 0;
}

// Helper function that iterates over a range of cached blocks, reading them from disk if needed.
// Uses the driver functions without checking; that is up to the caller.
static errno_t iterate_block_ranges(
    device_block_t *device,
    uint64_t        start_byte,
    uint64_t        byte_count,
    bool (*callback)(uint8_t *subblock_data, size_t subblock_len, size_t byte_offset, void __user *cookie),
    void __user *cookie,
    bool         mark_dirty
) {
    driver_block_t const *const driver = (void *)device->base.driver;
    assert_dev_keep(irq_disable());
    rcu_crit_enter();

    // Get the offsets in blocks.
    uint64_t       start_block = start_byte >> device->block_size_exp;
    uint64_t       end_block   = ((start_byte + byte_count - 1) >> device->block_size_exp) + 1;
    uint64_t const block_size  = 1llu << device->block_size_exp;

    size_t byte_offset = 0;
    for (uint64_t block = start_block; block < end_block; block++) {
        // Get sub-block offsets.
        uint64_t sub_offset;
        uint64_t sub_size;
        if (block == start_block) {
            sub_offset = start_byte & (block_size - 1);
            sub_size   = byte_count < block_size - sub_offset ? byte_count : block_size - sub_offset;
        } else {
            sub_offset = 0;
            sub_size   = byte_count - sub_offset < block_size ? byte_count - sub_offset : block_size;
        }

        // Read or allocate the cache entry.
        void *value;
        while (1) {
            // Get the value.
            void *raw_value = rtree_get(&device->cache, block);
            while (BLK_IS_LOCKED(raw_value)) {
                rcu_crit_exit();
                thread_yield();
                irq_disable();
                rcu_crit_enter();
                raw_value = rtree_get(&device->cache, block);
            }
            value = BLK_UNTAG(raw_value);
            if (value) {
                if (mark_dirty) {
                    // Try to mark the page as dirty.
                    if (raw_value != rtree_cmpxchg(&device->cache, block, raw_value, BLK_TAG_DIRTY(raw_value)).ptr) {
                        // Retry if doing so failed.
                        continue;
                    }
                }
                // It's resident in the cache; break out of this loop so it can be accessed.
                break;
            }

            // If NULL, try to lock so we can read the disk.
            errno_ptr_t res = rtree_cmpxchg(&device->cache, block, NULL, BLK_LOCKED_NULL);
            if (res.errno < 0) {
                rcu_crit_exit();
                irq_enable();
                return res.errno;
            }
            if (!BLK_IS_LOCKED(res.ptr)) {
                value = BLK_UNTAG(res.ptr);
                if (value) {
                    // Some other thread put this in the cache in the mean time.
                    break;
                }

                // We've locked an empty cache entry, now read from disk into a new entry.
                rcu_crit_exit();
                irq_enable();
                value = ENOMEM_ON_NULL(malloc(1 << device->block_size_exp), rtree_set(&device->cache, block, NULL));

                // Succeeded to allocate a cache entry, perform actual read.
                RETURN_ON_ERRNO(driver->read_blocks(device, block, 1, value), {
                    free(value);
                    rtree_set(&device->cache, block, NULL);
                });

                // Successfully read everything, now update the entry in the cache to this newly read one.
                rtree_set(&device->cache, block, mark_dirty ? BLK_TAG_DIRTY(value) : value);
                assert_dev_drop(rtree_get(&device->cache, block) == (mark_dirty ? BLK_TAG_DIRTY(value) : value));
                irq_disable();
                rcu_crit_enter();
                break;
            }
        }

        // Run the callback function with the acquired cache entry.
        if (callback(value + sub_offset, sub_size, byte_offset, cookie)) {
            // Access fault.
            rcu_crit_exit();
            irq_enable();
            return -EFAULT;
        }
        byte_offset += sub_size;
        start_byte  += sub_size;
        byte_count  -= sub_size;
    }

    rcu_crit_exit();
    irq_enable();
    return 0;
}

// Implementation of `device_block_write_bytes` after caching.
static bool write_bytes_cb(uint8_t *subblock_data, size_t subblock_len, size_t byte_offset, void __user *cookie) {
    uint8_t const __user *wdata = cookie;
    mmu_enable_sum();
    bool faulted = copy_from_user(subblock_data, (void const *)(wdata + byte_offset), subblock_len);
    mmu_disable_sum();
    return faulted;
}

// Write block device bytes.
// The alignment for DMA is handled by this function.
errno_t device_block_write_bytes(device_block_t *device, uint64_t offset, uint64_t size, void const __user *data) {
    if (offset + size < offset || offset + size > device->block_count << device->block_size_exp) {
        logkf(
            LOG_WARN,
            "OOB block device access rejected; writing 0x%{u64;x} @ 0x%{u64;x} on a 0x%{u64;x}*%{u64;d} device",
            offset,
            size,
            device->block_count,
            1llu << device->block_size_exp
        );
        return -EINVAL;
    }

    mutex_lock_shared(&device->base.driver_mtx);
    driver_block_t const *driver = (void *)device->base.driver;
    if (device->no_cache) {
        errno_t res = driver->write_bytes(device, offset, size, data);
        mutex_unlock_shared(&device->base.driver_mtx);
        return res;
    }

    errno_t res = iterate_block_ranges(device, offset, size, write_bytes_cb, (void __user *)data, true);

    mutex_unlock_shared(&device->base.driver_mtx);
    return res;
}

// Implementation of `device_block_read_bytes` after caching.
static bool read_bytes_cb(uint8_t *subblock_data, size_t subblock_len, size_t byte_offset, void __user *cookie) {
    uint8_t __user *rdata = cookie;
    mmu_enable_sum();
    bool faulted = copy_to_user((void *)(rdata + byte_offset), subblock_data, subblock_len);
    mmu_disable_sum();
    return faulted;
}

// Read block device bytes.
// The alignment for DMA is handled by this function.
errno_t device_block_read_bytes(device_block_t *device, uint64_t offset, uint64_t size, void __user *data) {
    if (offset + size < offset || offset + size > device->block_count << device->block_size_exp) {
        logkf(
            LOG_WARN,
            "OOB block device access rejected; reading 0x%{u64;x} @ 0x%{u64;x} on a 0x%{u64;x}*%{u64;d} device",
            offset,
            size,
            device->block_count,
            1llu << device->block_size_exp
        );
        return -EINVAL;
    }

    mutex_lock_shared(&device->base.driver_mtx);
    driver_block_t const *driver = (void *)device->base.driver;
    if (device->no_cache) {
        errno_t res = driver->read_bytes(device, offset, size, data);
        mutex_unlock_shared(&device->base.driver_mtx);
        return res;
    }

    errno_t res = iterate_block_ranges(device, offset, size, read_bytes_cb, data, false);

    mutex_unlock_shared(&device->base.driver_mtx);
    return res;
}

// Erase block device bytes.
errno_t device_block_erase_bytes(device_block_t *device, uint64_t offset, uint64_t size) {
    uint64_t const block_size  = 1llu << device->block_size_exp;
    uint64_t       end_offset  = offset + size;
    uint64_t       start_block = (offset + block_size - 1) >> device->block_size_exp;
    uint64_t       end_block   = end_offset >> device->block_size_exp;
    return device_block_erase_blocks(device, start_block, end_block - start_block);
}



// Apply all pending changes.
errno_t device_block_sync_all(device_block_t *device, bool flush) {
    return device_block_sync_blocks(device, 0, device->block_count, flush);
}

// Helper function that tries to sync all the blocks.
static errno_t
    sync_blocks_helper(device_block_t *device, size_t to_sync_len, uint64_t *to_sync_blocks, uint8_t **to_sync_data) {
    driver_block_t const *const driver = (void *)device->base.driver;
    rcu_crit_exit();
    irq_enable();

    for (size_t i = 0; i < to_sync_len; i++) {
        errno_t res = driver->write_blocks(device, to_sync_blocks[i], 1, BLK_UNTAG(to_sync_data[i]));
        if (res < 0) {
            // Failed to sync so write the remainder back as non-sync dirty.
            for (size_t x = i; x < to_sync_len; x++) {
                rtree_set(&device->cache, to_sync_blocks[x], BLK_TAG_DIRTY(BLK_UNTAG(to_sync_data[x])));
            }
            // Free only the entries that were actually synced.
            rcu_sync();
            for (size_t x = 0; x < i; x++) {
                free(BLK_UNTAG(to_sync_data[i]));
            }
            return res;
        }
        rtree_set(&device->cache, to_sync_blocks[i], NULL);
    }

    // Sync succeeded, free all the entries.
    rcu_sync();
    for (size_t i = 0; i < to_sync_len; i++) {
        free(BLK_UNTAG(to_sync_data[i]));
    }

    irq_disable();
    rcu_crit_enter();
    return 0;
}

// Apply pending changes in a range of blocks.
errno_t device_block_sync_blocks(device_block_t *device, uint64_t start, uint64_t count, bool flush) {
    mutex_lock_shared(&device->base.driver_mtx);
    if (!device->base.driver) {
        mutex_unlock_shared(&device->base.driver_mtx);
        return -ENOENT;
    }
    driver_block_t const *const driver = (void *)device->base.driver;

    // Number of blocks to sync.
    size_t   to_sync_len = 0;
    // Blocks that are going to be synced.
    uint64_t to_sync_blocks[BLK_SYNC_BATCH_SIZE];
    // Data arrays that are going to be synced.
    // Safe to use outside of RCU because the entries in the cache will be tagged as syncing.
    uint8_t *to_sync_data[BLK_SYNC_BATCH_SIZE];

    assert_dev_keep(irq_disable());
    rcu_crit_enter();

    rtree_iter_t iter = rtree_first(&device->cache, start);
    while (iter.node) {
        if (BLK_IS_LOCKED(iter.value) || BLK_IS_SYNC(iter.value) || (!flush && !BLK_IS_DIRTY(iter.value))) {
            iter = rtree_next(&device->cache, iter);
            continue;
        }

        // Evict clean entries if `flush` is `true`.
        if (flush && !BLK_IS_DIRTY(iter.value)) {
            // Must yield to proceed to the next RCU generation so garbage collection in `rtree_cmpxchg` doesn't hang.
            rcu_crit_exit();
            thread_yield();
            irq_disable();
            rcu_crit_enter();
            rtree_cmpxchg(&device->cache, iter.key, iter.value, NULL);
            iter = rtree_first(&device->cache, iter.key + 1);
            continue;
        }

        // Mark this block as syncing.
        if (iter.value != rtree_cmpxchg(&device->cache, iter.key, iter.value, BLK_TAG_SYNC(iter.value)).ptr) {
            // Pointer in the cache changed; try again.
            rcu_crit_exit();
            thread_yield();
            irq_disable();
            rcu_crit_enter();
            iter = rtree_first(&device->cache, iter.key);
            continue;
        }

        // Add it to the buffer of things that must sync.
        to_sync_data[to_sync_len]   = iter.value;
        to_sync_blocks[to_sync_len] = iter.key;
        to_sync_len++;
        if (to_sync_len < BLK_SYNC_BATCH_SIZE) {
            iter = rtree_next(&device->cache, iter);
            continue;
        }

        // Buffer filled up, sync more now.
        RETURN_ON_ERRNO(sync_blocks_helper(device, to_sync_len, to_sync_blocks, to_sync_data), {
            driver->sync_blocks(device, start, count);
            mutex_unlock_shared(&device->base.driver_mtx);
        });

        // Finally, continue looking for blocks to sync after this one.
        rtree_cmpxchg(&device->cache, iter.key, iter.value, NULL);
        iter        = rtree_first(&device->cache, iter.key + 1);
        to_sync_len = 0;
    }

    errno_t res = 0;
    if (to_sync_len) {
        res = sync_blocks_helper(device, to_sync_len, to_sync_blocks, to_sync_data);
    }

    rcu_crit_exit();
    irq_enable();

    if (driver->sync_blocks(device, start, count) < 0 && res >= 0) {
        res = -EIO;
    }
    mutex_unlock_shared(&device->base.driver_mtx);
    return res;
}

// Apply pending changes in a range of bytes.
errno_t device_block_sync_bytes(device_block_t *device, uint64_t offset, uint64_t size, bool flush) {
    if (device->no_cache) {
        mutex_lock_shared(&device->base.driver_mtx);
        if (!device->base.driver) {
            mutex_unlock_shared(&device->base.driver_mtx);
            return -ENOENT;
        }
        driver_block_t const *const driver = (void *)device->base.driver;
        errno_t                     res    = driver->sync_bytes(device, offset, size);
        mutex_unlock_shared(&device->base.driver_mtx);
        return res;
    }

    uint64_t const block_size  = 1llu << device->block_size_exp;
    size                      += offset & (block_size - 1);
    offset                    -= offset & (block_size - 1);
    if (size & (block_size - 1)) {
        size += block_size - (size & (block_size - 1));
    }
    return device_block_sync_blocks(device, offset >> device->block_size_exp, size >> device->block_size_exp, flush);
}
