
// SPDX-License-Identifier: MIT

#include "scheduler/scheduler.h"

#include "assertions.h"
#include "backtrace.h"
#include "badge_strings.h"
#include "cpulocal.h"
#include "isr_ctx.h"
#include "log.h"
#include "process/process.h"
#include "scheduler/cpu.h"
#include "scheduler/isr.h"
#if !CONFIG_NOMMU
#include "cpu/mmu.h"
#endif



// Requests the scheduler to prepare a switch from userland to kernel for a user thread.
// If `syscall` is true, copies registers `a0` through `a7` to the kernel thread.
// Sets the program counter for the thread to `pc`.
void sched_raise_from_isr(sched_thread_t *thread, bool syscall, void *entry_point) {
    assert_dev_drop(thread);
    assert_dev_drop(!(thread->flags & THREAD_KERNEL) && !(thread->flags & THREAD_PRIVILEGED));
    atomic_fetch_or(&thread->flags, THREAD_PRIVILEGED);

    // Set kernel thread entrypoint.
    thread->kernel_isr_ctx.regs.pc = (size_t)entry_point;
    thread->kernel_isr_ctx.regs.sp = thread->kernel_stack_top;
    thread->kernel_isr_ctx.regs.s0 = 0;
    thread->kernel_isr_ctx.regs.ra = 0;

    if (syscall) {
        // Copy syscall arg registers.
        thread->kernel_isr_ctx.regs.a0 = thread->user_isr_ctx.regs.a0;
        thread->kernel_isr_ctx.regs.a1 = thread->user_isr_ctx.regs.a1;
        thread->kernel_isr_ctx.regs.a2 = thread->user_isr_ctx.regs.a2;
        thread->kernel_isr_ctx.regs.a3 = thread->user_isr_ctx.regs.a3;
        thread->kernel_isr_ctx.regs.a4 = thread->user_isr_ctx.regs.a4;
        thread->kernel_isr_ctx.regs.a5 = thread->user_isr_ctx.regs.a5;
        thread->kernel_isr_ctx.regs.a6 = thread->user_isr_ctx.regs.a6;
        thread->kernel_isr_ctx.regs.a7 = thread->user_isr_ctx.regs.a7;
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

    // if (atomic_load(&process->flags) & PROC_EXITING) {
    //     // Request a context switch to a different thread.
    //     sched_request_switch_from_isr();
    // }
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
    size_t usp                    = thread->user_isr_ctx.regs.sp;
    size_t usize                  = sizeof(size_t) * 20;
    // if ((proc_map_contains_raw(thread->process, usp - usize, usize) & VMM_FLAG_RW) != VMM_FLAG_RW) {
    //     // Not enough stack that the process owns.
    //     return false;
    // }
    thread->user_isr_ctx.regs.sp -= usize;

    // Save context to user's stack.
#if !CONFIG_NOMMU
    mmu_enable_sum();
#endif
    size_t *stackptr = (size_t *)thread->user_isr_ctx.regs.sp;
    stackptr[0]      = thread->user_isr_ctx.regs.t0;
    stackptr[1]      = thread->user_isr_ctx.regs.t1;
    stackptr[2]      = thread->user_isr_ctx.regs.t2;
    stackptr[3]      = thread->user_isr_ctx.regs.a0;
    stackptr[4]      = thread->user_isr_ctx.regs.a1;
    stackptr[5]      = thread->user_isr_ctx.regs.a2;
    stackptr[6]      = thread->user_isr_ctx.regs.a3;
    stackptr[7]      = thread->user_isr_ctx.regs.a4;
    stackptr[8]      = thread->user_isr_ctx.regs.a5;
    stackptr[9]      = thread->user_isr_ctx.regs.a6;
    stackptr[10]     = thread->user_isr_ctx.regs.a7;
    stackptr[11]     = thread->user_isr_ctx.regs.t3;
    stackptr[12]     = thread->user_isr_ctx.regs.t4;
    stackptr[13]     = thread->user_isr_ctx.regs.t5;
    stackptr[14]     = thread->user_isr_ctx.regs.t6;
    stackptr[17]     = thread->user_isr_ctx.regs.pc;
    stackptr[18]     = thread->user_isr_ctx.regs.s0;
    stackptr[19]     = thread->user_isr_ctx.regs.ra;
#if !CONFIG_NOMMU
    mmu_disable_sum();
#endif

    // Set up registers for entering signal handler.
    thread->user_isr_ctx.regs.s0 = thread->user_isr_ctx.regs.sp + usize;
    thread->user_isr_ctx.regs.ra = return_vaddr;
    thread->user_isr_ctx.regs.pc = handler_vaddr;
    thread->user_isr_ctx.regs.a0 = signum;

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
    size_t usp   = thread->user_isr_ctx.regs.sp;
    size_t usize = sizeof(size_t) * 20;
    // if ((proc_map_contains_raw(thread->process, usp, usize) & VMM_FLAG_RW) != VMM_FLAG_RW) {
    //     // If this happens, the process probably corrupted it's own stack.
    //     return false;
    // }

// Restore user's state.
#if !CONFIG_NOMMU
    mmu_enable_sum();
#endif
    size_t *stackptr             = (size_t *)thread->user_isr_ctx.regs.sp;
    thread->user_isr_ctx.regs.t0 = stackptr[0];
    thread->user_isr_ctx.regs.t1 = stackptr[1];
    thread->user_isr_ctx.regs.t2 = stackptr[2];
    thread->user_isr_ctx.regs.a0 = stackptr[3];
    thread->user_isr_ctx.regs.a1 = stackptr[4];
    thread->user_isr_ctx.regs.a2 = stackptr[5];
    thread->user_isr_ctx.regs.a3 = stackptr[6];
    thread->user_isr_ctx.regs.a4 = stackptr[7];
    thread->user_isr_ctx.regs.a5 = stackptr[8];
    thread->user_isr_ctx.regs.a6 = stackptr[9];
    thread->user_isr_ctx.regs.a7 = stackptr[10];
    thread->user_isr_ctx.regs.t3 = stackptr[11];
    thread->user_isr_ctx.regs.t4 = stackptr[12];
    thread->user_isr_ctx.regs.t5 = stackptr[13];
    thread->user_isr_ctx.regs.t6 = stackptr[14];
    thread->user_isr_ctx.regs.pc = stackptr[17];
    thread->user_isr_ctx.regs.s0 = stackptr[18];
    thread->user_isr_ctx.regs.ra = stackptr[19];
#if !CONFIG_NOMMU
    mmu_disable_sum();
#endif

    // Restore user's stack pointer.
    thread->user_isr_ctx.regs.sp += usize;

    // Successfully returned from signal handler.
    return true;
}

// Return to exit the thread.
static void sched_exit_self(int code) {
    thread_exit(code);
}

// Prepares a context to be invoked as a kernel thread.
void sched_prepare_kernel_entry(sched_thread_t *thread, void *entry_point, void *arg) {
    // Initialize registers.
    mem_set(&thread->kernel_isr_ctx.regs, 0, sizeof(thread->kernel_isr_ctx.regs));
    thread->kernel_isr_ctx.regs.pc = (size_t)entry_point;
    thread->kernel_isr_ctx.regs.sp = thread->kernel_stack_top;
    thread->kernel_isr_ctx.regs.a0 = (size_t)arg;
    thread->kernel_isr_ctx.regs.ra = (size_t)sched_exit_self;
#if __riscv_xlen == 64
    asm("sd gp, %0" ::"m"(thread->kernel_isr_ctx.regs.gp));
#else
    asm("sw gp, %0" ::"m"(thread->kernel_isr_ctx.regs.gp));
#endif
}

// Prepares a pair of contexts to be invoked as a userland thread.
// Kernel-side in these threads is always started by an ISR and the entry point is given at that time.
void sched_prepare_user_entry(sched_thread_t *thread, size_t entry_point, size_t arg) {
    // Initialize kernel registers.
    mem_set(&thread->kernel_isr_ctx.regs, 0, sizeof(thread->kernel_isr_ctx.regs));
    thread->kernel_isr_ctx.regs.sp = thread->kernel_stack_top;
#if __riscv_xlen == 64
    asm("sd gp, %0" ::"m"(thread->kernel_isr_ctx.regs.gp));
#else
    asm("sw gp, %0" ::"m"(thread->kernel_isr_ctx.regs.gp));
#endif

    // This is duplicate info but the ISR assembly needs it to set up the stack.
    thread->user_isr_ctx.user_isr_stack = thread->kernel_stack_top;

    // Initialize userland registers.
    mem_set(&thread->user_isr_ctx.regs, 0, sizeof(thread->user_isr_ctx.regs));
    thread->user_isr_ctx.regs.pc = entry_point;
    thread->user_isr_ctx.regs.a0 = arg;
}

// Run arch-specific task switch code before `isr_context_switch`.
// Called after the scheduler decides what thread to switch to.
void sched_arch_task_switch(sched_thread_t *next) {
    (void)next;
}
