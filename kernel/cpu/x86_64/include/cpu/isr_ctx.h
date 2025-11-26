
// SPDX-License-Identifier: MIT

#pragma once

#include "cpu/regs.h"
#include "cpu/x86_msr.h"

#ifndef __ASSEMBLER__
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
typedef struct mpu_ctx      mpu_ctx_t;
typedef struct sched_thread sched_thread_t;
typedef struct isr_ctx      isr_ctx_t;
typedef struct cpulocal     cpulocal_t;
// Custom trap handler callback, returns true if the trap was suppressed.
typedef bool (*isr_noexc_cb_t)(isr_ctx_t *ctx, void *cookie);
#endif



#ifdef __ASSEMBLER__

#define STRUCT_BEGIN(structname) .equ structname##_t_size, 0
#define STRUCT_FIELD(structname, name, len)                                                                            \
    .equ structname##_t_##name, structname##_t_size;                                                                   \
    .equ structname##_t_size, structname##_t_size + len
#define STRUCT_FIELD_QWORD(structname, name)              STRUCT_FIELD(structname, name, 8)
#define STRUCT_FIELD_DWORD(structname, name)              STRUCT_FIELD(structname, name, 4)
#define STRUCT_FIELD_WORD(structname, name)               STRUCT_FIELD(structname, name, 2)
#define STRUCT_FIELD_PTR(structname, type, name)          STRUCT_FIELD(structname, name, 8)
#define STRUCT_FIELD_STRUCT(structname, type, name, size) STRUCT_FIELD(structname, name, size)
#define STRUCT_END(structname)

#else

#define STRUCT_BEGIN(structname)                                                                                       \
    typedef struct structname structname##_t;                                                                          \
    struct structname {
#define STRUCT_FIELD_QWORD(structname, name)              uint64_t name;
#define STRUCT_FIELD_DWORD(structname, name)              uint32_t name;
#define STRUCT_FIELD_WORD(structname, name)               uint16_t name;
#define STRUCT_FIELD_PTR(structname, type, name)          type *name;
#define STRUCT_FIELD_STRUCT(structname, type, name, size) type name;
#define STRUCT_END(structname)                                                                                         \
    }                                                                                                                  \
    ;

#endif



// Context for interrupts, exceptions and traps in relation to threads.
STRUCT_BEGIN(isr_ctx)
// Pointer to currently active memory protection information.
STRUCT_FIELD_PTR(isr_ctx, mpu_ctx_t, mpu_ctx)
// Frame pointer to use for backtraces.
STRUCT_FIELD_PTR(isr_ctx, void, frameptr)
// Registers storage.
// The trap/interrupt handler will save registers to here.
STRUCT_FIELD_STRUCT(isr_ctx, cpu_regs_t, regs, 176) // Size is 172 but align makes it 176
// Pointer to next isr_ctx to switch to.
// If nonnull, the trap/interrupt handler will context switch to this new context before exiting.
STRUCT_FIELD_PTR(isr_ctx, isr_ctx_t, ctxswitch)
// Pointer to owning sched_thread_t.
STRUCT_FIELD_PTR(isr_ctx, sched_thread_t, thread)
// Kernel context flags, only 32 bits available even on 64-bit targets.
STRUCT_FIELD_QWORD(isr_ctx, flags)
// Custom trap handler to call.
STRUCT_FIELD_STRUCT(isr_ctx, isr_noexc_cb_t, noexc_cb, 8)
// Cookie for custom trap handler.
STRUCT_FIELD_PTR(isr_ctx, void, noexc_cookie)
// Pointer to CPU-local struct.
STRUCT_FIELD_PTR(isr_ctx, cpulocal_t, cpulocal)
// Stack to use for syscalls.
STRUCT_FIELD_QWORD(isr_ctx, syscall_stack)
STRUCT_END(isr_ctx)

// `isr_ctx_t` flag: Is a kernel thread.
#define ISR_CTX_FLAG_KERNEL (1 << 0)
// `isr_ctx_t` flag: Currently running custom trap-handling code.
#define ISR_CTX_FLAG_NOEXC  (1 << 1)
// `isr_ctx_t` flag: Currently in a trap or interrupt handler.
#define ISR_CTX_FLAG_IN_ISR (1 << 2)
// `isr_ctx_t` flag: Currently in a double fault handler (which is fatal).
#define ISR_CTX_FLAG_2FAULT (1 << 3)


#ifndef __ASSEMBLER__
#include "cpulocal.h"
#include "mem/vmm.h"

// Stack alignment is defined to be 16 by the SYSV calling convention
#define STACK_ALIGNMENT 16

// Get the current ISR context.
static inline isr_ctx_t *isr_ctx_get() {
    return (isr_ctx_t *)msr_read(MSR_GSBASE);
}
// Get the outstanding context swap target, if any.
static inline isr_ctx_t *isr_ctx_switch_get() {
    isr_ctx_t *rdata;
    asm("mov %0, [gs:%1]" : "=r"(rdata) : "i"((size_t)offsetof(isr_ctx_t, ctxswitch)));
    return rdata;
}
// Set the context swap target to swap to before exiting the trap/interrupt handler.
static inline void isr_ctx_switch_set(isr_ctx_t *switch_to) {
    asm volatile("mov [gs:%1], %0" ::"r"(switch_to), "i"((size_t)offsetof(isr_ctx_t, ctxswitch)) : "memory");
}
// Immediately swap the ISR context handle.
static inline isr_ctx_t *isr_ctx_swap(isr_ctx_t *kctx) {
    isr_ctx_t *old = isr_ctx_get();
    msr_write(MSR_GSBASE, (uint64_t)kctx);
    asm volatile("mov [gs:%1], %0" ::"r"(old->ctxswitch), "i"((size_t)offsetof(isr_ctx_t, ctxswitch)) : "memory");
    asm volatile("mov [gs:%1], %0" ::"r"(old->cpulocal), "i"((size_t)offsetof(isr_ctx_t, cpulocal)) : "memory");
    return old;
}
// Print a register dump given isr_ctx_t.
void isr_ctx_dump(isr_ctx_t const *ctx);
// Print a register dump of the current registers.
void kernel_cur_regs_dump();
#endif



#undef STRUCT_BEGIN
#undef STRUCT_FIELD
#undef STRUCT_FIELD_WORD
#undef STRUCT_FIELD_DWORD
#undef STRUCT_FIELD_QWORD
#undef STRUCT_FIELD_PTR
#undef STRUCT_END
