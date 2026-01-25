
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// Enable interrupts if a condition is met.
static inline void irq_enable_if(bool enable) {
    if (enable) {
        asm volatile("sti" ::: "memory");
    }
}

// Disable interrupts if a condition is met.
static inline void irq_disable_if(bool disable) {
    if (disable) {
        asm volatile("cli" ::: "memory");
    }
}

// Enable interrupts.
static inline void irq_enable() {
    asm volatile("sti" ::: "memory");
}

// Query whether interrupts are enabled in this CPU.
static inline bool irq_is_enabled() {
    size_t flags;
    asm("pushfq;pop %0" : "=r"(flags));
    return flags & (1 << 9);
}

// Disable interrupts.
// Returns whether interrupts were enabled.
static inline bool irq_disable() {
    bool enabled = irq_is_enabled();
    asm volatile("cli" ::: "memory");
    return enabled;
}
