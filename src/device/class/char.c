
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "device/class/char.h"

#include "badgeros-abi/errno.h"
#include "sched/sync/mutex.h"



// Read bytes from the device.
errno_size_t device_char_read(device_char_t *device, void __user *rdata, size_t rdata_len, bool nonblock) {
    mutex_lock_shared(&device->base.driver_mtx);
    if (!device->base.driver) {
        mutex_unlock_shared(&device->base.driver_mtx);
        return -ENOENT;
    }
    driver_char_t const *driver = (void *)device->base.driver;
    errno_size_t         res    = driver->read(device, rdata, rdata_len, nonblock);
    mutex_unlock_shared(&device->base.driver_mtx);
    return res;
}

// Write bytes to the device.
errno_size_t device_char_write(device_char_t *device, void const __user *wdata, size_t wdata_len, bool nonblock) {
    mutex_lock_shared(&device->base.driver_mtx);
    if (!device->base.driver) {
        mutex_unlock_shared(&device->base.driver_mtx);
        return -ENOENT;
    }
    driver_char_t const *driver = (void *)device->base.driver;
    errno_size_t         res    = driver->write(device, wdata, wdata_len, nonblock);
    mutex_unlock_shared(&device->base.driver_mtx);
    return res;
}
