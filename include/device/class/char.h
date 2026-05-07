
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "badgeros-abi/errno.h"
#include "device/device.h"
#include "process/process.h"



// Character device.
typedef struct {
    device_t base;
} device_char_t;

// Character device driver functions.
typedef struct {
    driver_t base;
    // Read bytes from the device.
    errno_size_t (*read)(device_char_t *device, void __user *rdata, size_t rdata_len, bool nonblock);
    // Write bytes to the device.
    errno_size_t (*write)(device_char_t *device, void const __user *wdata, size_t wdata_len, bool nonblock);
    // Get current polling status flags.
    // Only ever called from Rust; `device` is `*mut device_char_t`, return is `u32` poll flags.
    uint32_t (*poll)(device_char_t *device);
    // Collect waitlists for the requested poll interest flags.
    // Only ever called from Rust; `collect` is an opaque `*mut Vec<&Waitlist>`.
    errno_t (*poll_waitlists)(device_char_t *device, uint32_t interest, void *collect);
} driver_char_t;

// Read bytes from the device.
errno_size_t device_char_read(device_char_t *device, void __user *rdata, size_t rdata_len, bool nonblock);
// Write bytes to the device.
errno_size_t device_char_write(device_char_t *device, void const __user *wdata, size_t wdata_len, bool nonblock);
