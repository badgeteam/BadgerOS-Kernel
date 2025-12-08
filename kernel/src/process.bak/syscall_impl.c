
// SPDX-License-Identifier: MIT

#include "process/syscall_impl.h"

#include "assertions.h"
#include "badge_strings.h"
#include "cpu/isr.h"
#include "errno.h"
#include "interrupt.h"
#include "process/internal.h"
#include "process/sighandler.h"
#include "process/types.h"
#include "rawprint.h"
#include "scheduler/cpu.h"
#include "signal.h"
#include "sys/wait.h"
#include "syscall_util.h"
#include "usercopy.h"
#if !CONFIG_NOMMU
#include "cpu/mmu.h"
#endif



// Map a new range of memory at an arbitrary virtual address.
// This may round up to a multiple of the page size.
// Alignment may be less than `align` if the kernel doesn't support it.
void *syscall_mem_map(void *vaddr_req, size_t min_size, size_t min_align, int flags) {
    return (void *)proc_map_raw(proc_current(), (size_t)vaddr_req, min_size, min_align, flags);
}

// Unmap a range of memory previously allocated with `SYSCALL_MEM_ALLOC`.
// Returns whether a range of memory was unmapped.
bool syscall_mem_unmap(void *address, size_t len) {
    return proc_unmap_raw(proc_current(), (size_t)address, len);
}



// Sycall: Exit the process; exit code can be read by parent process.
// When this system call returns, the thread will be suspended awaiting process termination.
void syscall_proc_exit(int code) {
    proc_exit_self(W_EXITED(code & 255));
}

// Get the command-line arguments (i.e. argc+argv) of a process (or pid 0 for self).
// If memory is large enough, a NULL-terminated argv array of C-string pointers and their data is stored in `memory`.
// The function returns how many bytes would be needed to store the structure.
// If the memory was not large enough, it it not modified.
size_t syscall_proc_getargs(size_t cap, void *memory) {
    process_t *const proc = proc_current();
    mutex_acquire_shared(&proc->mtx, TIMESTAMP_US_MAX);

    // Check required size.
    size_t required = proc->argv_size;
    if (cap >= required) {
        // Buffer fits; copy to user.
        sigsegv_assert(copy_to_user_raw(proc, (size_t)memory, proc->argv, required), (size_t)memory);
    }

    mutex_release_shared(&proc->mtx);
    return required;
}

// Create a "pre-start" child process that inherits this process' file descriptors and environment variables.
// The child process is created in a halted state and it is killed if the parent exits before the child is resumed.
// Returns process ID of new child on success, or a (negative) errno on failure.
int syscall_proc_pcreate(char const *binary, int argc, char const *const *argv) {
    process_t *const proc = proc_current();
    // TODO: Copy these strings.

    // Verify validity of pointers.
    if (strlen_from_user_raw(proc, (size_t)binary, PTRDIFF_MAX) < 0) {
        proc_sigsegv_handler((size_t)binary);
    }
    sigsys_assert(argc >= 0);
    if (!proc_map_contains_raw(proc, (size_t)argv, argc * sizeof(char const *))) {
        proc_sigsegv_handler((size_t)argv);
    }
    for (int i = 0; i < argc; i++) {
        if (strlen_from_user_raw(proc, (size_t)argv[i], PTRDIFF_MAX) < 0) {
            proc_sigsegv_handler((size_t)argv[i]);
        }
    }

    // The pointers are safe; create a process from them.
#if !CONFIG_NOMMU
    mmu_enable_sum();
#endif
    int res = proc_create(proc->pid, binary, argc, argv);
#if !CONFIG_NOMMU
    mmu_disable_sum();
#endif
    return res;
}

// Destroy a "pre-start" child process.
// Usually used in case of errors.
bool syscall_proc_pdestroy(int child) {
    if (!proc_is_parent(proc_current()->pid, child)) {
        return false;
    }
    return proc_delete_prestart(child);
}

// Starts a "pre-start" child process, thereby converting it into a running child process.
bool syscall_proc_pstart(int child) {
    // Start a process.
    if (proc_is_parent(proc_current()->pid, child) != 1) {
        return false;
    }
    return proc_start(child);
}

// Set the signal handler for a specific signal number.
void *syscall_proc_sighandler(int signum, void *newhandler) {
    sigsys_assert(signum >= 0 && signum < SIG_COUNT);
    process_t *const proc = proc_current();
    mutex_acquire(&proc->mtx, TIMESTAMP_US_MAX);
    void *old                 = (void *)proc->sighandlers[signum];
    proc->sighandlers[signum] = (size_t)newhandler;
    mutex_release(&proc->mtx);
    return old;
}

// Return from a signal handler.
void syscall_proc_sigret() {
    sigsys_assert(sched_is_sighandler());
    sigsegv_assert(sched_signal_exit(), 0);
    irq_disable();
    sched_lower_from_isr();
    isr_context_switch();
    assert_unreachable();
}

// Get child process status update.
NOASAN int syscall_proc_waitpid(int pid, int *wstatus, int options) {
    process_t *proc = proc_current();
    // Check memory ownership.
    sysutil_memassert_rw(wstatus, sizeof(int));

    while (!(options & WNOHANG)) {
        mutex_acquire_shared(&proc->mtx, TIMESTAMP_US_MAX);
        process_t *node     = (process_t *)proc->children.head;
        bool       eligible = false;
        while (node) {
            // Check whether child matches PID.
            bool node_eligible = false;
            if (pid == -1 || pid == 0) {
                node_eligible = true;
            } else if (pid > 0) {
                node_eligible = node->pid == pid;
            }
            // Check whether node matches the filters.
            bool node_matches = node_eligible;
            if (WIFCONTINUED(node->state_code)) {
                node_matches = options & WCONTINUED;
            } else if (WIFSTOPPED(node->state_code)) {
                node_matches = options & WUNTRACED;
            }
            // Try to claim state change from child.
            if (node_matches && (atomic_fetch_and(&node->flags, ~PROC_STATECHG) & PROC_STATECHG)) {
                if (wstatus) {
                    *wstatus = node->state_code;
                }
                int  pid    = node->pid;
                bool exited = node->flags & PROC_EXITED;
                mutex_release_shared(&proc->mtx);
                if (exited && !(options & WNOWAIT)) {
                    proc_delete(pid);
                }
                return pid;
            }
            // Not found; next node.
            node      = (process_t *)node->node.next;
            eligible |= node_eligible;
        }
        mutex_release_shared(&proc->mtx);
        if (!eligible) {
            // No children with matching PIDs exist.
            return -ECHILD;
        }
        thread_yield();
    }

    // Nothing found in non-blocking wait.
    return 0;
}



// Temporary write system call.
void syscall_temp_write(char const *message, size_t length) {
    sysutil_memassert_r(message, length);
    mutex_acquire(&log_mtx, TIMESTAMP_US_MAX);
#if !CONFIG_NOMMU
    mmu_enable_sum();
#endif
    rawprint_substr(message, length);
#if !CONFIG_NOMMU
    mmu_disable_sum();
#endif
    mutex_release(&log_mtx);
}
