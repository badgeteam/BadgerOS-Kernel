
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "device/class/irqctl.h"



// Maximum number of PLIC contexts.
#define PLIC_MAX_CTX_COUNT 15872
// Address space size of the PLIC.
#define PLIC_MEM_SIZE      0x4000000

// Offset for interrupt priorities.
#define PLIC_PRIO_OFF        0x000000
// Offset for interrupt pending bits.
#define PLIC_PENDING_OFF     0x001000
// Offset for interrupt enable bits.
#define PLIC_ENABLE_OFF(ctx) (0x002000 + (ctx) * 0x80)
// Offset for priority threshold.
#define PLIC_THRESH_OFF(ctx) (0x200000 + (ctx) * 0x1000)
// Offset for claim/complete.
#define PLIC_CLAIM_OFF(ctx)  (0x200004 + (ctx) * 0x1000)



// RISC-V PLIC device cookie.
typedef struct {
    // Number of implemented interrupts.
    size_t ndev;
    // PLIC context to use per logical CPU.
    int   *ctx_by_cpu;
} device_riscv_plic_t;



extern driver_irqctl_t const riscv_plic_driver;
