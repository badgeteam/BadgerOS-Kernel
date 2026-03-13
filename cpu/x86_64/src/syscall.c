
// SPDX-License-Identifier: MIT

#include "isr_ctx.h"



// Helper function that swaps from user to kernel ISR context on syscall.
void amd64_syscall_raise() {
    isr_ctx_t *cur = isr_ctx_get();
    isr_ctx_swap(&cur->thread->kernel_isr_ctx);
}

// Helper function that swaps from kernel ISR context to user ISR context on syscall return.
void amd64_syscall_lower() {
    isr_ctx_t *cur = isr_ctx_get();
    isr_ctx_swap(&cur->thread->user_isr_ctx);
}
