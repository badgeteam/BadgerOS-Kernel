
// SPDX-License-Identifier: MIT

#pragma once

#include "cpu/regs.h"
#include "isr_ctx.h"

#ifndef __ASSEMBLER__
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#endif

#ifdef __ASSEMBLER__
// clang-format off

// Called from ASM on interrupt.
    .global amd64_interrupt_handler
// Called from ASM on trap.
    .global amd64_trap_handler

// clang-format on
#else
// Callback from ASM to platform-specific interrupt handler.
extern void amd64_interrupt_handler();
// ASM system call wrapper function.
extern void amd64_syscall_wrapper();
// Callback from ASM on non-syscall trap.
extern void amd64_trap_handler(size_t trapno, size_t error_code);

// Explicit context switch from kernel.
// Interrupts must be disabled on entry and will be re-enabled on exit.
// If the context switch target is not set, this is a NOP.
extern void        isr_context_switch();
// Pause the CPU briefly.
static inline void isr_pause() {
    // TODO.
}

#endif
