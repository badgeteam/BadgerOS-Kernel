
// SPDX-License-Identifier: MIT

#pragma once

#include "cpu/regs.h"
#include "cpu/riscv.h"

#include <stdbool.h>

// Enable interrupts if a condition is met.
static inline void irq_enable_if(bool enable) {
    long mask = enable << CSR_STATUS_IE_BIT;
    asm volatile("csrs " CSR_STATUS_STR ", %0" ::"ri"(mask));
}

// Disable interrupts if a condition is met.
static inline void irq_disable_if(bool disable) {
    long mask = disable << CSR_STATUS_IE_BIT;
    asm volatile("csrc " CSR_STATUS_STR ", %0" ::"ri"(mask));
}

// Enable interrupts.
static inline void irq_enable() {
    long mask = 1 << CSR_STATUS_IE_BIT;
    asm volatile("csrs " CSR_STATUS_STR ", %0" ::"ri"(mask));
}

// Disable interrupts.
// Returns whether interrupts were enabled.
static inline bool irq_disable() {
    long mask = 1 << CSR_STATUS_IE_BIT;
    asm volatile("csrrc %0, " CSR_STATUS_STR ", %0" : "+r"(mask));
    return (mask >> CSR_STATUS_IE_BIT) & 1;
}

// Query whether interrupts are enabled in this CPU.
static inline bool irq_is_enabled() {
    long mask;
    asm("csrr %0, " CSR_STATUS_STR : "=r"(mask));
    return (mask >> CSR_STATUS_IE_BIT) & 1;
}
