
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "device/device.h"



// AHCI controller device.
typedef struct {
    device_t base;
} device_ahci_t;

// AHCI controller device driver functions.
typedef struct {
    driver_t base;
} driver_ahci_t;
