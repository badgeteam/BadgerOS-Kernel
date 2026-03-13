
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "device/device.h"
#include "process/process.h"
#include "radixtree.h"

#include <malloc.h>



// Describes a single partition.
typedef struct {
    // On-disk byte offset.
    uint64_t offset;
    // On-disk byte size.
    uint64_t size;
    // Partition type.
    uint64_t type[2];
    // Partition UUID.
    uint64_t uuid[2];
    // Partition name.
    char    *name;
    // Partition name length.
    size_t   name_len;
    // Partition is read-only.
    bool     readonly;
} partition_t;

// Describes the partitioning system on a particular volume.
typedef struct {
    // Array of partitions.
    partition_t *parts;
    // Number of partitions.
    size_t       parts_len;
    // Volume label / name.
    char        *name;
    // Volume label length.
    size_t       name_len;
    // Disk UUID.
    uint64_t     uuid[2];
} volume_info_t;

// Return value of `get_volume_info`.
typedef struct {
    // Volume info.
    volume_info_t info;
    // Error code or boolean of whether `info` is valid.
    errno_bool_t  errno;
} get_volume_info_t;

// Block device.
typedef struct {
    device_t      base;
    // Number of blocks.
    uint64_t      block_count;
    // Device DMA alignment requirement; no more than block size.
    size_t        dma_align;
    // Log-base-2 of device block size.
    uint8_t       block_size_exp;
    // Native erased byte value.
    uint8_t       erase_value;
    // Fast read; do not cache read data, only write data; requires byte read access.
    // If `false`, all accesses use entire blocks.
    bool          no_read_cache;
    // Do not cache at all; requires byte write/erase access.
    // If `false`, all accesses use entire blocks.
    bool          no_cache;
    // Block cache, if any.
    rtree_t       cache;
    // Mutex guarding the volume info.
    mutex_t       volume_info_mtx;
    // Cache of stored volume info; read automatically after activation of a block device.
    volume_info_t volume_info;
    // Indev number among devices with the same prefix (e.g. 0 for "sata0").
    int           blk_index;
} device_block_t;

// Block device driver functions.
typedef struct {
    driver_t    base;
    // Name under which block nodes in a devtmpfs are created.
    char const *blk_node_name;
    // Write device blocks.
    // The caller must ensure that `data` is aligned at least as much as needed for DMA.
    errno_t (*write_blocks)(device_block_t *device, uint64_t start, uint64_t count, void const *data);
    // Read device blocks.
    // The caller must ensure that `data` is aligned at least as much as needed for DMA.
    errno_t (*read_blocks)(device_block_t *device, uint64_t start, uint64_t count, void *data);
    // Test whether a single block is erased with native erase value.
    errno_t (*is_block_erased)(device_block_t *device, uint64_t block);
    // Sync disk's write caches for blocks.
    errno_t (*sync_blocks)(device_block_t *device, uint64_t start, uint64_t count);
    // Erase blocks.
    errno_t (*erase_blocks)(device_block_t *device, uint64_t start, uint64_t count);
    // [optional] Write device bytes.
    errno_t (*write_bytes)(device_block_t *device, uint64_t start, uint64_t count, void const __user *data);
    // [optional] Read device bytes.
    errno_t (*read_bytes)(device_block_t *device, uint64_t start, uint64_t count, void __user *data);
    // [optional] Erase bytes.
    errno_t (*erase_bytes)(device_block_t *device, uint64_t start, uint64_t count);
    // [optional] Sync disk's write caches for bytes.
    errno_t (*sync_bytes)(device_block_t *device, uint64_t start, uint64_t count);
} driver_block_t;

// TODO: Clone function for volume info?

// Free a `partition_t`.
static inline void partition_free(partition_t part) {
    free(part.name);
}

// Free a `partition_list_t`.
static inline void volume_info_free(volume_info_t info) {
    for (size_t i = 0; i < info.parts_len; i++) {
        partition_free(info.parts[i]);
    }
    free(info.name);
}



// (Re-)scan partitions for this drive.
errno_t device_block_scan_parts(device_block_t *device);

// Write device blocks.
// The alignment for DMA is handled by this function.
errno_t device_block_write_blocks(device_block_t *device, uint64_t start, uint64_t count, void const *data);
// Read device blocks.
// The alignment for DMA is handled by this function.
errno_t device_block_read_blocks(device_block_t *device, uint64_t start, uint64_t count, void *data);
// Erase blocks.
errno_t device_block_erase_blocks(device_block_t *device, uint64_t start, uint64_t count);
// Write block device bytes.
// The alignment for DMA is handled by this function.
errno_t device_block_write_bytes(device_block_t *device, uint64_t offset, uint64_t size, void const __user *data);
// Read block device bytes.
// The alignment for DMA is handled by this function.
errno_t device_block_read_bytes(device_block_t *device, uint64_t offset, uint64_t size, void __user *data);
// Erase block device bytes.
errno_t device_block_erase_bytes(device_block_t *device, uint64_t offset, uint64_t size);

// Apply all pending changes.
// If `flush` is `true`, will remove the cache entries.
errno_t device_block_sync_all(device_block_t *device, bool flush);
// Apply pending changes in a range of blocks.
// If `flush` is `true`, will remove the cache entries.
errno_t device_block_sync_blocks(device_block_t *device, uint64_t start, uint64_t count, bool flush);
// Apply pending changes in a range of bytes.
// If `flush` is `true`, will remove the cache entries.
errno_t device_block_sync_bytes(device_block_t *device, uint64_t offset, uint64_t size, bool flush);
