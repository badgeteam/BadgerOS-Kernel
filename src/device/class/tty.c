
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "device/class/tty.h"

#include "sched/sync/mutex.h"

// Try to set the given terminal attributes.
errno_t device_tty_setattr(device_tty_t *device, struct termios const *newattr) {
    mutex_lock_shared(&device->base.base.driver_mtx);
    if (!device->base.base.driver) {
        mutex_unlock_shared(&device->base.base.driver_mtx);
        return -ENOENT;
    }
    driver_tty_t const *driver = (void *)device->base.base.driver;
    mutex_lock(&device->tio_mtx);
    errno_t res = driver->setattr(device, newattr);
    if (res == 0) {
        device->tio = *newattr;
    }
    mutex_unlock(&device->tio_mtx);
    mutex_unlock_shared(&device->base.base.driver_mtx);
    return res;
}

// Get the current terminal attributes.
errno_t device_tty_getattr(device_tty_t *device, struct termios *oldattr) {
    mutex_lock_shared(&device->base.base.driver_mtx);
    if (!device->base.base.driver) {
        mutex_unlock_shared(&device->base.base.driver_mtx);
        return -ENOENT;
    }
    mutex_lock_shared(&device->tio_mtx);
    *oldattr = device->tio;
    mutex_unlock_shared(&device->tio_mtx);
    mutex_unlock_shared(&device->base.base.driver_mtx);
    return 0;
}
