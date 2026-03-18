
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "badgeros-abi/termios.h"
#include "device/class/char.h"
#include "device/device.h"
#include "sched/sync/mutex.h"



// TTY device.
typedef struct {
    device_char_t  base;
    mutex_t        tio_mtx;
    struct termios tio;
} device_tty_t;

// TTY device driver functions.
typedef struct {
    driver_char_t base;
    // Try to set the given terminal attributes.
    // If successful, `tio` will be be updated by the device subsytem.
    errno_t (*setattr)(device_tty_t *device, struct termios const *newattr);
} driver_tty_t;

// Try to set the given terminal attributes.
errno_t device_tty_setattr(device_tty_t *device, struct termios const *newattr);
// Get the current terminal attributes.
errno_t device_tty_getattr(device_tty_t *device, struct termios *oldattr);
