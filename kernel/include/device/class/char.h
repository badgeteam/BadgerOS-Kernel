
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "device/device.h"
#include "errno.h"
#include "process/process.h"



// Character device.
typedef struct {
    device_t base;
} device_char_t;

// Character device driver functions.
typedef struct {
    driver_t base;
    // Read bytes from the device.
    errno_size_t (*read)(device_char_t *device, void __user *rdata, size_t rdata_len);
    // Write bytes to the device.
    errno_size_t (*write)(device_char_t *device, void const __user *wdata, size_t wdata_len);
} driver_char_t;

// Read bytes from the device.
errno_size_t device_char_read(device_char_t *device, void __user *rdata, size_t rdata_len);
// Write bytes to the device.
errno_size_t device_char_write(device_char_t *device, void const __user *wdata, size_t wdata_len);
