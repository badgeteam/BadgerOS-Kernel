
// SPDX-License-Identifier: MIT

#include "scheduler/scheduler.h"

#include "assertions.h"
#include "backtrace.h"
#include "badge_strings.h"
#include "cpu/priv_level.h"
#include "cpu/segmentation.h"
#include "isr_ctx.h"
#include "log.h"
#include "process/internal.h"
#include "process/process.h"
#include "process/types.h"
#include "scheduler/cpu.h"
#include "scheduler/isr.h"
#if !CONFIG_NOMMU
#include "cpu/mmu.h"
#endif



// Requests the scheduler to prepare a switch from userland to kernel for a user thread.
// If `syscall` is true, copies registers `a0` through `a7` to the kernel thread.
// Sets the program counter for the thread to `pc`.
void sched_raise_from_isr(sched_thread_t *thread, bool syscall, void *entry_point) {
    assert_dev_drop(!(thread->flags & THREAD_KERNEL) && !(thread->flags & THREAD_PRIVILEGED));
    thread->flags |= THREAD_PRIVILEGED;

    // Set up kernel entrypoint.
    thread->kernel_isr_ctx.regs.rip = (size_t)entry_point;
    thread->kernel_isr_ctx.regs.rsp = thread->kernel_stack_top;

    if (syscall) {
        // Transfer syscall arguments.
        thread->kernel_isr_ctx.regs.rdi = thread->user_isr_ctx.regs.rdi;
        thread->kernel_isr_ctx.regs.rsi = thread->user_isr_ctx.regs.rsi;
        thread->kernel_isr_ctx.regs.rdx = thread->user_isr_ctx.regs.rdx;
        thread->kernel_isr_ctx.regs.rcx = thread->user_isr_ctx.regs.rcx;
        thread->kernel_isr_ctx.regs.r8  = thread->user_isr_ctx.regs.r8;
        thread->kernel_isr_ctx.regs.r9  = thread->user_isr_ctx.regs.r9;
    }

    // Do time accounting.
    timestamp_us_t    now            = time_us();
    sched_cpulocal_t *info           = isr_ctx_get()->cpulocal->sched;
    timestamp_us_t    used           = now - info->last_preempt;
    thread->timeusage.cycle_time    += used;
    thread->timeusage.user_time     += used;
    info->last_preempt               = now;
    thread->kernel_isr_ctx.cpulocal  = isr_ctx_get()->cpulocal;

    // Set context switch target to kernel thread.
    isr_ctx_switch_set(&thread->kernel_isr_ctx);
}

// Requests the scheduler to prepare a switch from kernel to userland for a user thread.
// Resumes the userland thread where it left off.
void sched_lower_from_isr() {
    sched_thread_t *thread  = sched_current_thread();
    process_t      *process = thread->process;
    assert_dev_drop(!(thread->flags & THREAD_KERNEL) && (thread->flags & THREAD_PRIVILEGED));
    atomic_fetch_and(&thread->flags, ~THREAD_PRIVILEGED);

    // Do time accounting.
    timestamp_us_t    now          = time_us();
    sched_cpulocal_t *info         = isr_ctx_get()->cpulocal->sched;
    timestamp_us_t    used         = now - info->last_preempt;
    thread->timeusage.cycle_time  += used;
    thread->timeusage.kernel_time += used;
    info->last_preempt             = now;

    // Set context switch target to user thread.
    isr_ctx_switch_set(&thread->user_isr_ctx);
    assert_dev_drop(!(thread->user_isr_ctx.flags & ISR_CTX_FLAG_KERNEL));

    if (atomic_load(&process->flags) & PROC_EXITING) {
        // Request a context switch to a different thread.
        sched_request_switch_from_isr();
    }
}

// Check whether the current thread is in a signal handler.
// Returns signal number, or 0 if not in a signal handler.
bool sched_is_sighandler() {
    sched_thread_t *thread = sched_current_thread();
    return atomic_load(&thread->flags) & THREAD_SIGHANDLER;
}

// Enters a signal handler in the current thread.
// Returns false if there isn't enough resources to do so.
bool sched_signal_enter(size_t handler_vaddr, size_t return_vaddr, int signum) {
    sched_thread_t *thread = sched_current_thread();

    // Ensure the user has enough stack.
    size_t usp   = thread->user_isr_ctx.regs.rsp;
    size_t usize = sizeof(size_t) * 20;
    if ((proc_map_contains_raw(thread->process, usp - usize, usize) & VMM_FLAG_RW) != VMM_FLAG_RW) {
        // Not enough stack that the process owns.
        return false;
    }
    thread->user_isr_ctx.regs.rsp -= usize;

    // Save context to user's stack.
#if !CONFIG_NOMMU
    mmu_enable_sum();
#endif
    size_t *stackptr = (size_t *)thread->user_isr_ctx.regs.rsp;
    // TODO: Save caller-save registers.
#if !CONFIG_NOMMU
    mmu_disable_sum();
#endif

    // Set up registers for entering signal handler.
    // TODO.

    // Successfully entered signal handler.
    atomic_fetch_or(&thread->flags, THREAD_SIGHANDLER);
    return true;
}

// Exits a signal handler in the current thread.
// Returns false if the process cannot be resumed.
bool sched_signal_exit() {
    sched_thread_t *thread = sched_current_thread();
    if (!(atomic_fetch_and(&thread->flags, ~THREAD_SIGHANDLER) & THREAD_SIGHANDLER)) {
        return false;
    }

    // Ensure the user still has the stack.
    size_t usp   = thread->user_isr_ctx.regs.rsp;
    size_t usize = sizeof(size_t) * 20;
    if ((proc_map_contains_raw(thread->process, usp, usize) & VMM_FLAG_RW) != VMM_FLAG_RW) {
        // If this happens, the process probably corrupted it's own stack.
        return false;
    }

// Restore user's state.
#if !CONFIG_NOMMU
    mmu_enable_sum();
#endif
    size_t *stackptr = (size_t *)thread->user_isr_ctx.regs.rsp;
    // TODO: Restore user's caller-save registers.
#if !CONFIG_NOMMU
    mmu_disable_sum();
#endif

    // Restore user's stack pointer.
    // TODO.

    // Successfully returned from signal handler.
    return true;
}

// Return to exit the thread.
static NAKED void sched_exit_self() {
    // clang-format off
    asm(
        "mov %rdi, %rax;"
        "call thread_exit;"
    );
    // clang-format on
}

// Prepares a context to be invoked as a kernel thread.
void sched_prepare_kernel_entry(sched_thread_t *thread, void *entry_point, void *arg) {
    // Initialize registers.
    mem_set(&thread->kernel_isr_ctx.regs, 0, sizeof(thread->kernel_isr_ctx.regs));
    thread->kernel_isr_ctx.regs.gsbase = (size_t)&thread->kernel_isr_ctx;
    thread->kernel_isr_ctx.regs.rip    = (size_t)entry_point;
    thread->kernel_isr_ctx.regs.rsp    = thread->kernel_stack_top - sizeof(size_t);
    thread->kernel_isr_ctx.regs.cs     = FORMAT_SEGMENT(SEGNO_KCODE, 0, PRIV_KERNEL);
    thread->kernel_isr_ctx.regs.ds     = FORMAT_SEGMENT(SEGNO_KDATA, 0, PRIV_KERNEL);
    thread->kernel_isr_ctx.regs.es     = FORMAT_SEGMENT(SEGNO_KDATA, 0, PRIV_KERNEL);
    thread->kernel_isr_ctx.regs.fs     = FORMAT_SEGMENT(SEGNO_KDATA, 0, PRIV_KERNEL);
    thread->kernel_isr_ctx.regs.gs     = FORMAT_SEGMENT(SEGNO_KDATA, 0, PRIV_KERNEL);
    thread->kernel_isr_ctx.regs.ss     = FORMAT_SEGMENT(SEGNO_KDATA, 0, PRIV_KERNEL);
    thread->kernel_isr_ctx.regs.rflags = RFLAGS_AC;

    // Set to return to `sched_exit_self`, which will use its return value as the thread exit code.
    *(size_t *)thread->kernel_isr_ctx.regs.rsp = (size_t)sched_exit_self;
}

// Prepares a pair of contexts to be invoked as a userland thread.
// Kernel-side in these threads is always started by an ISR and the entry point is given at that time.
void sched_prepare_user_entry(sched_thread_t *thread, size_t entry_point, size_t arg) {
    // Initialize kernel registers.
    mem_set(&thread->kernel_isr_ctx.regs, 0, sizeof(thread->user_isr_ctx.regs));
    thread->kernel_isr_ctx.regs.gsbase = (size_t)&thread->kernel_isr_ctx;
    thread->kernel_isr_ctx.regs.cs     = FORMAT_SEGMENT(SEGNO_KCODE, 0, PRIV_KERNEL);
    thread->kernel_isr_ctx.regs.ds     = FORMAT_SEGMENT(SEGNO_KDATA, 0, PRIV_KERNEL);
    thread->kernel_isr_ctx.regs.es     = FORMAT_SEGMENT(SEGNO_KDATA, 0, PRIV_KERNEL);
    thread->kernel_isr_ctx.regs.fs     = FORMAT_SEGMENT(SEGNO_KDATA, 0, PRIV_KERNEL);
    thread->kernel_isr_ctx.regs.gs     = FORMAT_SEGMENT(SEGNO_KDATA, 0, PRIV_KERNEL);
    thread->kernel_isr_ctx.regs.ss     = FORMAT_SEGMENT(SEGNO_KDATA, 0, PRIV_KERNEL);
    thread->kernel_isr_ctx.regs.rflags = RFLAGS_AC;
    mem_set(&thread->user_isr_ctx.regs, 0, sizeof(thread->user_isr_ctx.regs));
    thread->user_isr_ctx.regs.gsbase   = (size_t)&thread->user_isr_ctx;
    thread->user_isr_ctx.regs.rip      = entry_point;
    thread->user_isr_ctx.regs.rsp      = thread->kernel_stack_top;
    thread->user_isr_ctx.regs.cs       = FORMAT_SEGMENT(SEGNO_UCODE, 0, PRIV_USER);
    thread->user_isr_ctx.regs.ds       = FORMAT_SEGMENT(SEGNO_UDATA, 0, PRIV_USER);
    thread->user_isr_ctx.regs.es       = FORMAT_SEGMENT(SEGNO_UDATA, 0, PRIV_USER);
    thread->user_isr_ctx.regs.fs       = FORMAT_SEGMENT(SEGNO_UDATA, 0, PRIV_USER);
    thread->user_isr_ctx.regs.gs       = FORMAT_SEGMENT(SEGNO_UDATA, 0, PRIV_USER);
    thread->user_isr_ctx.regs.ss       = FORMAT_SEGMENT(SEGNO_UDATA, 0, PRIV_USER);
    thread->user_isr_ctx.regs.rflags   = PRIV_USER << RFLAGS_IOPL_BASE;
    thread->user_isr_ctx.syscall_stack = thread->kernel_stack_top;
}

// Run arch-specific task switch code before `isr_context_switch`.
// Called after the scheduler decides what thread to switch to.
void sched_arch_task_switch(sched_thread_t *next) {
    size_t *ptr = (void *)(isr_ctx_get()->cpulocal->arch.tss + TSS_STACK);
    *ptr        = next->kernel_stack_top;
}
