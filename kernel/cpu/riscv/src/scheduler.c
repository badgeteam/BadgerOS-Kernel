
// SPDX-License-Identifier: MIT

#include "scheduler/scheduler.h"

#include "assertions.h"
#include "badge_strings.h"
#include "cpulocal.h"
#include "isr_ctx.h"
#include "process/process.h"
#include "scheduler/cpu.h"
#include "scheduler/isr.h"
#include "signal.h"
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

    if (atomic_load(proc_flags(process)) & PROC_FLAG_STOPPING) {
        // Request a context switch to a different thread.
        sched_request_switch_from_isr();
    }
}

// Enters a signal handler in the current thread.
// Returns false if there isn't enough resources to do so.
bool sched_signal_enter(size_t handler_vaddr, size_t return_vaddr, siginfo_t siginfo) {
    sched_thread_t *thread = sched_current_thread();

    // Save context to user's stack.
    ucontext_t stackframe;
    thread->user_isr_ctx.regs.sp -= sizeof(stackframe);

    stackframe.siginfo = siginfo;
    stackframe.regs.t0 = thread->user_isr_ctx.regs.t0;
    stackframe.regs.t1 = thread->user_isr_ctx.regs.t1;
    stackframe.regs.t2 = thread->user_isr_ctx.regs.t2;
    stackframe.regs.a0 = thread->user_isr_ctx.regs.a0;
    stackframe.regs.a1 = thread->user_isr_ctx.regs.a1;
    stackframe.regs.a2 = thread->user_isr_ctx.regs.a2;
    stackframe.regs.a3 = thread->user_isr_ctx.regs.a3;
    stackframe.regs.a4 = thread->user_isr_ctx.regs.a4;
    stackframe.regs.a5 = thread->user_isr_ctx.regs.a5;
    stackframe.regs.a6 = thread->user_isr_ctx.regs.a6;
    stackframe.regs.a7 = thread->user_isr_ctx.regs.a7;
    stackframe.regs.t3 = thread->user_isr_ctx.regs.t3;
    stackframe.regs.t4 = thread->user_isr_ctx.regs.t4;
    stackframe.regs.t5 = thread->user_isr_ctx.regs.t5;
    stackframe.regs.t6 = thread->user_isr_ctx.regs.t6;
    stackframe.regs.pc = thread->user_isr_ctx.regs.pc;
    stackframe.regs.s0 = thread->user_isr_ctx.regs.s0;
    stackframe.regs.ra = thread->user_isr_ctx.regs.ra;

#if !CONFIG_NOMMU
    mmu_enable_sum();
#endif
    size_t *stackptr = (size_t *)thread->user_isr_ctx.regs.sp;
    bool    faulted  = isr_noexc_mem_copy(stackptr, &stackframe, sizeof(stackframe));
#if !CONFIG_NOMMU
    mmu_disable_sum();
#endif
    if (faulted) {
        return false;
    }

    // Set up registers for entering signal handler.
    thread->user_isr_ctx.regs.s0 = thread->user_isr_ctx.regs.sp + sizeof(stackframe);
    thread->user_isr_ctx.regs.ra = return_vaddr;
    thread->user_isr_ctx.regs.pc = handler_vaddr;
    thread->user_isr_ctx.regs.a0 = siginfo.si_signo;
    thread->user_isr_ctx.regs.a1 = thread->user_isr_ctx.regs.sp;
    thread->user_isr_ctx.regs.a2 = thread->user_isr_ctx.regs.sp;

    // Successfully entered signal handler.
    return true;
}

// Exits a signal handler in the current thread.
// Returns false if the process cannot be resumed.
bool sched_signal_exit() {
    sched_thread_t *thread = sched_current_thread();

    ucontext_t stackframe;
    size_t    *stackptr = (size_t *)thread->user_isr_ctx.regs.sp;

#if !CONFIG_NOMMU
    mmu_enable_sum();
#endif
    bool faulted = isr_noexc_mem_copy(&stackframe, stackptr, sizeof(stackframe));
#if !CONFIG_NOMMU
    mmu_disable_sum();
#endif
    if (faulted) {
        return false;
    }

    // Restore user's state.
    thread->user_isr_ctx.regs.t0 = stackframe.regs.t0;
    thread->user_isr_ctx.regs.t1 = stackframe.regs.t1;
    thread->user_isr_ctx.regs.t2 = stackframe.regs.t2;
    thread->user_isr_ctx.regs.a0 = stackframe.regs.a0;
    thread->user_isr_ctx.regs.a1 = stackframe.regs.a1;
    thread->user_isr_ctx.regs.a2 = stackframe.regs.a2;
    thread->user_isr_ctx.regs.a3 = stackframe.regs.a3;
    thread->user_isr_ctx.regs.a4 = stackframe.regs.a4;
    thread->user_isr_ctx.regs.a5 = stackframe.regs.a5;
    thread->user_isr_ctx.regs.a6 = stackframe.regs.a6;
    thread->user_isr_ctx.regs.a7 = stackframe.regs.a7;
    thread->user_isr_ctx.regs.t3 = stackframe.regs.t3;
    thread->user_isr_ctx.regs.t4 = stackframe.regs.t4;
    thread->user_isr_ctx.regs.t5 = stackframe.regs.t5;
    thread->user_isr_ctx.regs.t6 = stackframe.regs.t6;
    thread->user_isr_ctx.regs.pc = stackframe.regs.pc;
    thread->user_isr_ctx.regs.s0 = stackframe.regs.s0;
    thread->user_isr_ctx.regs.ra = stackframe.regs.ra;

    // Restore user's stack pointer.
    thread->user_isr_ctx.regs.sp += sizeof(stackframe);

    // Successfully returned from signal handler.
    return true;
}

// Prepares a context to be invoked as a kernel thread.
void sched_prepare_kernel_entry(sched_thread_t *thread, void *entry_point, void *arg) {
    // Initialize registers.
    mem_set(&thread->kernel_isr_ctx.regs, 0, sizeof(thread->kernel_isr_ctx.regs));
    thread->kernel_isr_ctx.regs.pc = (size_t)entry_point;
    thread->kernel_isr_ctx.regs.sp = thread->kernel_stack_top;
    thread->kernel_isr_ctx.regs.a0 = (size_t)arg;
    thread->kernel_isr_ctx.regs.ra = (size_t)thread_exit;
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
    thread->user_isr_ctx.regs.sp = thread->user_stack_top;
}

// Run arch-specific task switch code before `isr_context_switch`.
// Called after the scheduler decides what thread to switch to.
void sched_arch_task_switch(sched_thread_t *next) {
    (void)next;
}
