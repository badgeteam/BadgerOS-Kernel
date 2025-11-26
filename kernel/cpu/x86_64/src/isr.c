
// SPDX-License-Identifier: MIT

#include "cpu/isr.h"

#include "assertions.h"
#include "backtrace.h"
#include "cpu/isr_ctx.h"
#include "interrupt.h"
#include "log.h"
#include "panic.h"
#include "process/internal.h"
#include "process/sighandler.h"
#include "process/types.h"
#include "rawprint.h"
#include "scheduler/cpu.h"
#include "scheduler/types.h"
#if !CONFIG_NOMMU
#include "mem/vmm.h"
#endif



// Trap names.
static char const *const trapnames[] = {
    "Division error",
    "Debug trap",
    "Non-maskable interrupt",
    "Breakpoint",
    "Overflow",
    "Bound range exceeded",
    "Invalid opcode",
    "Device not available",
    "Double fault",
    NULL, // Coprocessor segment overrun.
    "Invalid TSS",
    "Segment not present",
    "Stack-segment fault",
    "General protection fault",
    "Page fault",
    NULL, // Reserved.
    NULL, // x87 FP exception.
    NULL, // Alignment check.
    "Machine check",
    "SIMD floating-point exception",
    NULL, // Virtualization exception.
    "Control protection exception",
    NULL, // Reserved.
    NULL, // Reserved.
    NULL, // Reserved.
    NULL, // Reserved.
    NULL, // Reserved.
    NULL, // Reserved.
    NULL, // Hypervisor injection exception.
    "VMM communication exception",
    "Security exception",
};
enum { TRAPNAMES_LEN = sizeof(trapnames) / sizeof(trapnames[0]) };

// Kill a process from a trap / ISR.
static void kill_proc_on_trap() {
    proc_exit_self(-1);
    irq_disable();
    sched_lower_from_isr();
    isr_context_switch();
    assert_unreachable();
}

// Generic interrupt handler that runs all callbacks on an IRQ.
void generic_interrupt_handler(int irq);

// Called from ASM on non-system call trap.
void amd64_trap_handler(size_t trapno, size_t error_code) {
    if (trapno >= 32) {
        generic_interrupt_handler(trapno - 32);
        return;
    }

    isr_ctx_t recurse_ctx;
    recurse_ctx.mem_ctx  = NULL;
    recurse_ctx.flags    = ISR_CTX_FLAG_IN_ISR | ISR_CTX_FLAG_KERNEL;
    isr_ctx_t *kctx      = isr_ctx_swap(&recurse_ctx);
    recurse_ctx.thread   = kctx->thread;
    recurse_ctx.cpulocal = kctx->cpulocal;

    // Double fault detection.
    bool fault3 = kctx->flags & ISR_CTX_FLAG_2FAULT;
    bool fault2 = kctx->flags & ISR_CTX_FLAG_IN_ISR;
    if (fault2) {
        kctx->flags |= ISR_CTX_FLAG_2FAULT;
    }

    // Check for custom trap handler.
    if (!fault2 && (kctx->flags & ISR_CTX_FLAG_NOEXC) && kctx->noexc_cb(kctx, kctx->noexc_cookie)) {
        isr_ctx_swap(kctx);
        return;
    }

    // Unhandled trap.
    claim_panic();
    rawprint("\033[0m");
    if (fault3) {
        rawprint("**** TRIPLE FAULT ****\n");
        panic_poweroff();
    } else if (fault2) {
        rawprint("**** DOUBLE FAULT ****\n");
    }

    // Print trap name.
    if (trapno < TRAPNAMES_LEN && trapnames[trapno]) {
        rawprint(trapnames[trapno]);
    } else {
        rawprint("Exception 0x");
        rawprinthex(trapno, 2);
    }
    rawprint(" at PC 0x");
    rawprinthex(kctx->regs.rip, 16);
    rawputc('\n');

    // Memory addresses.
    if (trapno == X86_EXC_PF) {
        rawprint("While accessing 0x");
        size_t vaddr;
        asm("mov %0, %%cr2" : "=r"(vaddr));
        rawprinthex(vaddr, sizeof(size_t) * 2);
        rawputc('\n');

        virt2phys_t info = vmm_virt2phys(kctx->mem_ctx, vaddr);
        if (info.valid) {
            rawprint("Memory at this address: ");
            rawputc(info.flags & VMM_FLAG_R ? 'r' : '-');
            rawputc(info.flags & VMM_FLAG_W ? 'w' : '-');
            rawputc(info.flags & VMM_FLAG_X ? 'x' : '-');
            rawputc(info.flags & VMM_FLAG_U ? 'u' : 'k');
            rawputc(info.flags & VMM_FLAG_G ? 'g' : '-');
            rawprint("\nPhysical address: 0x");
            rawprinthex(info.paddr, 2 * sizeof(size_t));
            rawputc('\n');
        } else {
            rawprint("No memory at this address.\n");
        }
    }


    // Print privilige mode.
    if (!fault2) {
        bool is_k = (kctx->regs.cs & 3) == 0;
        rawprint("Running in ");
        rawprint(is_k ? "kernel mode" : "user mode");
        if (is_k != !!(kctx->flags & ISR_CTX_FLAG_KERNEL)) {
            rawprint(" (despite is_kernel_thread=");
            rawputc((kctx->flags & ISR_CTX_FLAG_KERNEL) ? '1' : '0');
            rawputc(')');
        }
    }

    // Print current process.
    if (!fault2 && kctx->thread && !(kctx->thread->flags & THREAD_KERNEL)) {
        rawprint(" in process ");
        rawprintdec(kctx->thread->process->pid, 1);
    }
    rawputc('\n');
    backtrace_from_ptr(kctx->frameptr);

    isr_ctx_dump(kctx);
    panic_poweroff_unchecked();
}

// Return a value from the syscall handler.
void syscall_return(long long value) {
    sched_thread_t *thread = isr_ctx_get()->thread;
    isr_ctx_t      *usr    = &thread->user_isr_ctx;

    usr->regs.rax = value;
    usr->regs.rip = usr->regs.rcx;

    if (proc_signals_pending_raw(thread->process)) {
        proc_signal_handler();
    }
    irq_disable();
    sched_lower_from_isr();
    isr_context_switch();
    assert_unreachable();
}
