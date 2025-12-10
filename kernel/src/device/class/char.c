
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "device/class/char.h"

#include "errno.h"



// Read bytes from the device.
errno_size_t device_char_read(device_char_t *device, void __user *rdata, size_t rdata_len) {
    mutex_acquire_shared(&device->base.driver_mtx, TIMESTAMP_US_MAX);
    driver_char_t const *driver = (void *)device->base.driver;
    errno_size_t         res    = driver->read(device, rdata, rdata_len);
    mutex_release_shared(&device->base.driver_mtx);
    return res;
}

// Write bytes to the device.
errno_size_t device_char_write(device_char_t *device, void const __user *wdata, size_t wdata_len) {
    mutex_acquire_shared(&device->base.driver_mtx, TIMESTAMP_US_MAX);
    driver_char_t const *driver = (void *)device->base.driver;
    errno_size_t         res    = driver->write(device, wdata, wdata_len);
    mutex_release_shared(&device->base.driver_mtx);
    return res;
}
