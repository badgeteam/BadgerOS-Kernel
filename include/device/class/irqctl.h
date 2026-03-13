
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "device/device.h"



// Interrupt controller device.
typedef struct {
    device_t base;
} device_irqctl_t;

// Interrupt controller driver functions.
typedef struct {
    driver_t base;
} driver_irqctl_t;
