
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// Interrupt handler for the INTC to forward external interrupts to.
extern void (*intc_ext_irq_handler)();
