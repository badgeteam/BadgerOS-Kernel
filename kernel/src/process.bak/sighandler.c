
// SPDX-License-Identifier: MIT

#include "process/sighandler.h"

#include "assertions.h"
#include "backtrace.h"
#include "cpu/isr.h"
#include "interrupt.h"
#include "log.h"
#include "malloc.h"
#include "mem/vmm.h"
#include "process/internal.h"
#include "process/types.h"
#include "scheduler/cpu.h"
#include "sys/wait.h"
#if !CONFIG_NOMMU
#include "cpu/mmu.h"
#endif

// Signal name table.
char const *signames[SIG_COUNT] = {
    [SIGHUP] = "SIGHUP",   [SIGINT] = "SIGINT",       [SIGQUIT] = "SIGQUIT", [SIGILL] = "SIGILL",
    [SIGTRAP] = "SIGTRAP", [SIGABRT] = "SIGABRT",     [SIGBUS] = "SIGBUS",   [SIGFPE] = "SIGFPE",
    [SIGKILL] = "SIGKILL", [SIGUSR1] = "SIGUSR1",     [SIGSEGV] = "SIGSEGV", [SIGUSR2] = "SIGUSR2",
    [SIGPIPE] = "SIGPIPE", [SIGALRM] = "SIGALRM",     [SIGTERM] = "SIGTERM", [SIGSTKFLT] = "SIGSTKFLT",
    [SIGCHLD] = "SIGCHLD", [SIGCONT] = "SIGCONT",     [SIGSTOP] = "SIGSTOP", [SIGTSTP] = "SIGTSTP",
    [SIGTTIN] = "SIGTTIN", [SIGTTOU] = "SIGTTOU",     [SIGURG] = "SIGURG",   [SIGXCPU] = "SIGXCPU",
    [SIGXFSZ] = "SIGXFSZ", [SIGVTALRM] = "SIGVTALRM", [SIGPROF] = "SIGPROF", [SIGWINCH] = "SIGWINCH",
    [SIGIO] = "SIGIO",     [SIGPWR] = "SIGPWR",       [SIGSYS] = "SIGSYS",
};

// Show memmap info for a virtual address.
static inline void memmap_info(process_t *const proc, size_t vaddr) {
    logkf_from_isr(LOG_INFO, "While accessing 0x%{size;x}", vaddr);
#if !CONFIG_NOMMU
    uint32_t flags = vmm_virt2phys(&proc->memmap.mem_ctx, vaddr).flags;
#else
    uint32_t flags = proc_map_contains_raw(proc, vaddr, 1);
#endif
    if (flags & VMM_FLAG_RWX) {
        char tmp[6] = {0};
        tmp[0]      = flags & VMM_FLAG_R ? 'r' : '-';
        tmp[1]      = flags & VMM_FLAG_W ? 'w' : '-';
        tmp[2]      = flags & VMM_FLAG_X ? 'x' : '-';
        tmp[3]      = flags & VMM_FLAG_U ? 'u' : 'k';
        tmp[4]      = flags & VMM_FLAG_G ? 'g' : '-';
        logkf_from_isr(LOG_INFO, "Memory at this address: %{cs}", tmp);
    } else {
        logk_from_isr(LOG_INFO, "No memory at this address.");
    }
}

// Runs the appropriate handler for a signal.
static void run_sighandler(int signum, uint64_t cause) {
    sched_thread_t  *thread = sched_current_thread();
    process_t *const proc   = thread->process;
    // Check for signal handler.
    if (signum == SIGKILL || proc->sighandlers[signum] == SIG_DFL) {
        if ((SIG_DFL_KILL_MASK >> signum) & 1) {
            // Process didn't catch a signal that kills it.
            mutex_acquire(&log_mtx, TIMESTAMP_US_MAX);
            logkf_from_isr(LOG_ERROR, "Process %{d} received %{cs}", proc->pid, signames[signum]);
            if (signum == SIGSEGV) {
                memmap_info(proc, cause);
            }
            // Print backtrace of the calling thread.
#if !CONFIG_NOMMU
            mmu_enable_sum();
#endif
#ifdef __riscv
            backtrace_from_ptr((void *)thread->user_isr_ctx.regs.s0);
#elif defined(__x86_86__)
            backtrace_from_ptr((void *)thread->user_isr_ctx.regs.rbp);
#endif
#if !CONFIG_NOMMU
            mmu_disable_sum();
#endif
            isr_ctx_dump(&thread->user_isr_ctx);
            mutex_release(&log_mtx);
            // Finally, kill the process.
            proc_exit_self(W_SIGNALLED(signum));
        }
    } else if (proc->sighandlers[signum]) {
        sched_signal_enter(proc->sighandlers[signum], proc->sighandlers[0], signum);
    }
}

// Kernel side of the signal handler.
// Called in the kernel side of a user thread when a signal might be queued.
void proc_signal_handler() {
    process_t *const proc = proc_current();
    mutex_acquire(&proc->mtx, TIMESTAMP_US_MAX);
    if (proc->sigpending.len) {
        // Pop the first pending signal and run its handler.
        sigpending_t *node = (sigpending_t *)dlist_pop_front(&proc->sigpending);
        mutex_release(&proc->mtx);
        run_sighandler(node->signum, 0);
        free(node);
    } else {
        mutex_release(&proc->mtx);
    }
    irq_disable();
    sched_lower_from_isr();
    isr_context_switch();
    assert_unreachable();
}

// Raises a fault signal to the current thread.
// If the thread is already running a signal handler, the process is killed.
static void trap_signal_handler(int signum, uint64_t cause) NORETURN;
static void trap_signal_handler(int signum, uint64_t cause) {
    sched_thread_t  *thread  = sched_current_thread();
    process_t *const proc    = thread->process;
    int              current = sched_is_sighandler();
    if (current) {
        // If the thread is still running a signal handler, terminate the process.
        mutex_acquire(&log_mtx, TIMESTAMP_US_MAX);
        if (current > 0 && current < SIG_COUNT && signames[current]) {
            logkf_from_isr(
                LOG_ERROR,
                "Process %{d} received %{cs} while handling %{cs}",
                proc->pid,
                signames[signum],
                signames[current]
            );
        } else {
            logkf_from_isr(
                LOG_ERROR,
                "Process %{d} received %{cs} while handling Signal #%{d}",
                proc->pid,
                signames[signum],
                current
            );
        }
        if (signum == SIGSEGV) {
            memmap_info(proc, cause);
        }
        // Print backtrace of the calling thread.
#if !CONFIG_NOMMU
        mmu_enable_sum();
#endif
#ifdef __riscv
        backtrace_from_ptr((void *)thread->user_isr_ctx.regs.s0);
#elif defined(__x86_86__)
        backtrace_from_ptr((void *)thread->user_isr_ctx.regs.rbp);
#endif
#if !CONFIG_NOMMU
        mmu_disable_sum();
#endif
        isr_ctx_dump(&thread->user_isr_ctx);
        mutex_release(&log_mtx);
        // Finally, kill the process.
        proc_exit_self(W_SIGNALLED(signum));

    } else {
        // If the thread isn't running a signal handler, run the appropriate one.
        run_sighandler(signum, cause);
    }
    irq_disable();
    sched_lower_from_isr();
    isr_context_switch();
    assert_unreachable();
}

// Raises a segmentation fault to the current thread.
// Called in the kernel side of a used thread when hardware detects a segmentation fault.
void proc_sigsegv_handler(size_t vaddr) {
    trap_signal_handler(SIGSEGV, vaddr);
}

// Raises an illegal instruction fault to the current thread.
// Called in the kernel side of a used thread when hardware detects an illegal instruction fault.
void proc_sigill_handler() {
    trap_signal_handler(SIGILL, 0);
}

// Raises a trace/breakpoint trap to the current thread.
// Called in the kernel side of a used thread when hardware detects a trace/breakpoint trap.
void proc_sigtrap_handler() {
    trap_signal_handler(SIGTRAP, 0);
}

// Raises an invalid system call to the current thread.
// Called in the kernel side of a used thread when a system call does not exist.
void proc_sigsys_handler() {
    trap_signal_handler(SIGSYS, 0);
}
