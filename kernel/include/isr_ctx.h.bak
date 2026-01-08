
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct isr_ctx isr_ctx_t;

// Get the current ISR context.
static inline isr_ctx_t *isr_ctx_get();
// Get the outstanding context switch target, if any.
static inline isr_ctx_t *isr_ctx_switch_get();
// Set the context switch target to switch to before exiting the trap/interrupt handler.
static inline void       isr_ctx_switch_set(isr_ctx_t *switch_to);

// Print a register dump given isr_ctx_t.
void isr_ctx_dump(isr_ctx_t const *ctx);
// Print a register dump of the current registers.
void kernel_cur_regs_dump();

#include "cpu/isr_ctx.h"

// Function to run with `isr_noexc_run`.
typedef void (*isr_noexc_t)(void *cookie);
// Trap handler to run with `isr_noexc_run`.
typedef void (*isr_catch_t)(void *cookie, cpu_regs_t *regs_ptr);
// Run a restricted function and catch exceptions.
// The code will run with interrupts disabled.
// All traps will cause the optional `trap_handler` to be called and `code` to terminate early.
// This should only be used for debug or ISA detection purposes.
// Returns whether a trap occurred.
bool isr_noexc_run(isr_noexc_t code, isr_catch_t trap_handler, void *cookie);

// Try to copy memory from src to dest.
// Returns whether an access trap occurred.
bool isr_noexc_mem_copy(void *dest, void const *src, size_t len);
// Try to copy uint8_t from src to dest.
// Returns whether an access trap occurred.
bool isr_noexc_copy_u8(uint8_t *dest, uint8_t const *src);
// Try to copy uint16_t from src to dest.
// Returns whether an access trap occurred.
bool isr_noexc_copy_u16(uint16_t *dest, uint16_t const *src);
// Try to copy uint32_t from src to dest.
// Returns whether an access trap occurred.
bool isr_noexc_copy_u32(uint32_t *dest, uint32_t const *src);
// Try to copy uint64_t from src to dest.
// Returns whether an access trap occurred.
bool isr_noexc_copy_u64(uint64_t *dest, uint64_t const *src);

// Try to copy size_t from src to dest.
// Returns whether an access trap occurred.
static inline bool isr_noexc_copy_size(size_t *dest, size_t const *src) {
#if __SIZE_WIDTH__ == 64
    return isr_noexc_copy_u64(dest, src);
#else
    return isr_noexc_copy_u32(dest, src);
#endif
}
