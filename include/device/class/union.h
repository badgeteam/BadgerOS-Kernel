
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "block.h"
#include "device/class/pcictl.h"
#include "irqctl.h"
#include "tty.h"



// Union of all device classes.
typedef union {
    device_t        base;
    device_block_t  block;
    device_irqctl_t irqctl;
    device_tty_t    tty;
    device_pcictl_t pcictl;
} device_union_t;
