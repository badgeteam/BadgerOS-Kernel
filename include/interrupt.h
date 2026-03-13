
// SPDX-License-Identifier: MIT

#pragma once

#include "cpu/interrupt.h"

typedef struct isr_ctx isr_ctx_t;
// Interrupt service routine functions.
typedef void (*isr_t)(int irq, void *cookie);
// Installed ISR (opaque struct).
typedef struct isr_entry isr_entry_t;
// Reference to installed ISR.
typedef isr_entry_t     *isr_handle_t;

// Initialise interrupt drivers for this CPU.
void irq_init(isr_ctx_t *tmp_ctx);

// Enable the IRQ.
void         irq_ch_enable(int irq);
// Disable the IRQ.
void         irq_ch_disable(int irq);
// Query whether the IRQ is enabled.
bool         irq_ch_is_enabled(int irq);
// Add an ISR to a certain IRQ.
isr_handle_t isr_install(int irq, isr_t isr_func, void *cookie);
// Remove an ISR.
void         isr_remove(isr_handle_t handle);

// Enable interrupts if a condition is met.
static inline void irq_enable_if(bool enable);
// Disable interrupts if a condition is met.
static inline void irq_disable_if(bool disable);
// Enable interrupts.
static inline void irq_enable();
// Disable interrupts.
// Returns whether interrupts were enabled.
static inline bool irq_disable();
// Query whether interrupts are enabled on this CPU.
static inline bool irq_is_enabled();
