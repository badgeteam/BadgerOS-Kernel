// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "device/device.h"



typedef struct CpuLocal cpulocal_t;



void cpulocal_set_irqctl(int smp_idx, device_t *device);
