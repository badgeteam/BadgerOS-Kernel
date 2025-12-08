
// SPDX-License-Identifier: MIT

#include "process/process.h"

#include "arrays.h"
#include "assertions.h"
#include "badge_strings.h"
#include "housekeeping.h"
#include "isr_ctx.h"
#include "kbelf.h"
#include "log.h"
#include "malloc.h"
#include "mem/vmm.h"
#include "mutex.h"
#include "panic.h"
#include "process/internal.h"
#include "process/sighandler.h"
#include "process/types.h"
#include "scheduler/cpu.h"
#include "scheduler/scheduler.h"
#include "sys/wait.h"
#include "time.h"
#include "todo.h"
#include "usercopy.h"

#include <stdatomic.h>


// Globally unique PID number counter.
static pid_t      pid_counter = 1;
// Global process lifetime mutex.
mutex_t           proc_mtx    = MUTEX_T_INIT_SHARED;
// Number of processes.
size_t            procs_len   = 0;
// Capacity for processes.
size_t            procs_cap   = 0;
// Process array.
process_t       **procs       = NULL;
extern atomic_int kernel_shutdown_mode;
// Allow process 1 to die without kernel panic.
static bool       allow_proc1_death() {
    // While the kernel is shutting down and init is the only process left.
    return kernel_shutdown_mode && procs_len == 1;
}

// Set arguments for a process.
// If omitted, argc will be 0 and argv will be NULL.
static bool proc_setargs_raw_unsafe(process_t *process, int argc, char const *const *argv);
// Create a new thread in a process.
// Returns created user thread handle or -errno.
static long proc_create_thread_raw_unlocked(process_t *process, size_t entry_point, size_t arg, int priority);



// Send a signal to all running processes in the system except the init process.
void proc_signal_all(int signal) {
    mutex_acquire_shared(&proc_mtx, TIMESTAMP_US_MAX);
    for (size_t i = 0; i < procs_len; i++) {
        if (procs[i]->pid == 1)
            continue;
        proc_raise_signal_raw(procs[i], signal);
    }
    mutex_release_shared(&proc_mtx);
}

// Whether any non-init processes are currently running.
bool proc_has_noninit() {
    mutex_acquire_shared(&proc_mtx, TIMESTAMP_US_MAX);
    for (size_t i = 0; i < procs_len; i++) {
        if (procs[i]->pid == 1)
            continue;
        if (atomic_load(&procs[i]->flags) & PROC_RUNNING) {
            mutex_release_shared(&proc_mtx);
            return true;
        }
    }
    mutex_release_shared(&proc_mtx);
    return false;
}



// Clean up: the housekeeping task.
static void clean_up_from_housekeeping(int taskno, void *arg) {
    (void)taskno;
    mutex_acquire_shared(&proc_mtx, TIMESTAMP_US_MAX);
    process_t *proc = proc_get_unsafe((int)(ptrdiff_t)arg);

    // Delete run-time resources.
    proc_delete_runtime_raw(proc);
    if (!proc->parent) {
        // Init process during shutdown; delete right away.
        mutex_release_shared(&proc_mtx);
        proc_delete((int)(ptrdiff_t)arg);
        return;
    }

    // Check whether parent ignores SIGCHLD.
    mutex_acquire_shared(&proc->parent->mtx, TIMESTAMP_US_MAX);
    bool ignored = proc->parent->sighandlers[SIGCHLD] == SIG_IGN;
    mutex_release_shared(&proc->parent->mtx);

    if (ignored) {
        // Parent process ignores SIGCHLD; delete right away.
        mutex_release_shared(&proc_mtx);
        proc_delete((int)(ptrdiff_t)arg);
    } else {
        // Signal parent process.
        atomic_fetch_or(&proc->flags, PROC_STATECHG);
        proc_raise_signal_raw(proc->parent, SIGCHLD);
        mutex_release_shared(&proc_mtx);
    }
}

// Kill a process from one of its own threads.
void proc_exit_self(int code) {
    // Mark this process as exiting.
    sched_thread_t *thread  = sched_current_thread();
    process_t      *process = thread->process;
    mutex_acquire(&process->mtx, TIMESTAMP_US_MAX);
    atomic_fetch_or(&process->flags, PROC_EXITING);
    process->state_code = code;
    mutex_release(&process->mtx);

    // Add deleting runtime to the housekeeping list.
    assert_always(hk_add_once(0, clean_up_from_housekeeping, (void *)(long)process->pid) != -1);

    // Stop this thread to prevent `sched_lower_from_isr` from running dead user code.
    thread_exit(code);
}


// Compare processes by ID.
int proc_sort_pid_cmp(void const *a, void const *b) {
    return (*(process_t **)a)->pid - (*(process_t **)b)->pid;
}

// Create a new, empty process.
errno_procptr_t proc_create_raw(pid_t parentpid, char const *binary, int argc, char const *const *argv) {
    // Get a new PID.
    mutex_acquire(&proc_mtx, TIMESTAMP_US_MAX);
    process_t *parent = proc_get_unsafe(parentpid);
    if (pid_counter == 1) {
        assert_dev_drop(parentpid <= 0);
    } else if (!parent) {
        mutex_release(&proc_mtx);
        return (errno_procptr_t){-ENOENT, NULL};
    }

    // Allocate a process entry.
    process_t *handle = malloc(sizeof(process_t));
    if (!handle) {
        mutex_release(&proc_mtx);
        return (errno_procptr_t){-ENOMEM, NULL};
    }

    // Install default values.
    *handle = (process_t){
        .node        = DLIST_NODE_EMPTY,
        .parent      = parent,
        .binary      = binary,
        .argc        = 0,
        .argv        = NULL,
        .argv_size   = 0,
        .fds_len     = 0,
        .fds         = NULL,
        .threads_len = 0,
        .threads     = NULL,
        .pid         = pid_counter,
        .memmap =
            {
                .regions_len = 0,
#if !CONFIG_NOMMU
                .regions_cap = 0,
                .regions     = NULL,
#endif
            },
        .mtx        = MUTEX_T_INIT_SHARED,
        .flags      = PROC_PRESTART,
        .sigpending = DLIST_EMPTY,
        .children   = DLIST_EMPTY,
    };

    // Set default signal handlers.
    for (size_t i = 0; i < SIG_COUNT; i++) {
        handle->sighandlers[i] = SIG_DFL;
    }

    // Install arguments.
    if (!proc_setargs_raw_unsafe(handle, argc, argv)) {
        free(handle);
        mutex_release(&proc_mtx);
        return (errno_procptr_t){-ENOMEM, NULL};
    }

    // Initialise the empty memory map.
    errno_t pt_res = vmm_create_user_ctx(&handle->memmap.mem_ctx);
    if (pt_res < 0) {
        mutex_release(&proc_mtx);
        return (errno_procptr_t){pt_res, NULL};
    }

    // Insert the entry into the list.
    array_binsearch_t res = array_binsearch(procs, sizeof(process_t *), procs_len, &handle, proc_sort_pid_cmp);
    if (!array_lencap_insert(&procs, sizeof(process_t *), &procs_len, &procs_cap, NULL, res.index)) {
        free(handle->argv);
        free(handle);
        mutex_release(&proc_mtx);
        return (errno_procptr_t){-ENOMEM, NULL};
    }
    procs[res.index] = handle;
    pid_counter++;

    // Add to the parent process' child list.
    if (parent) {
        mutex_acquire(&parent->mtx, TIMESTAMP_US_MAX);
        dlist_append(&parent->children, &handle->node);
        mutex_release(&parent->mtx);
    }

    mutex_release(&proc_mtx);
    return (errno_procptr_t){0, handle};
}

// Get a process handle by ID.
process_t *proc_get(pid_t pid) {
    mutex_acquire_shared(&proc_mtx, TIMESTAMP_US_MAX);
    process_t *res = proc_get_unsafe(pid);
    mutex_release_shared(&proc_mtx);
    return res;
}

// Look up a process without locking the global process mutex.
process_t *proc_get_unsafe(pid_t pid) {
    process_t         dummy     = {.pid = pid};
    process_t        *dummy_ptr = &dummy;
    array_binsearch_t res       = array_binsearch(procs, sizeof(process_t *), procs_len, &dummy_ptr, proc_sort_pid_cmp);
    return res.found ? procs[res.index] : NULL;
}

// Get the process' flags.
uint32_t proc_getflags_raw(process_t *process) {
    return atomic_load(&process->flags);
}

// Get a handle to the current process, if any.
process_t *proc_current() {
    sched_thread_t *thread = sched_current_thread();
    return thread ? thread->process : NULL;
}

// Get the PID of the current process, if any.
pid_t proc_current_pid() {
    process_t *proc = proc_current();
    return proc ? proc->pid : 0;
}

// Set arguments for a process.
// If omitted, argc will be 0 and argv will be NULL.
static bool proc_setargs_raw_unsafe(process_t *process, int argc, char const *const *argv) {
    // Measure required memory for argv.
    size_t required = sizeof(size_t) * ((size_t)argc + 1);
    for (size_t i = 0; i < (size_t)argc; i++) {
        required += cstr_length(argv[i]) + 1;
    }

    // Allocate memory for the argv.
    char *mem = realloc(process->argv, required);
    if (!mem) {
        return false;
    }

    // Copy datas into the argv.
    size_t off = sizeof(size_t) * ((size_t)argc + 1);
    for (size_t i = 0; i < (size_t)argc; i++) {
        // Argument pointer.
        *(char **)(mem + sizeof(size_t) * i) = (mem + off);

        // Copy in the string.
        size_t len = cstr_length(argv[i]);
        mem_copy(mem + off, argv[i], len + 1);
        off += len;
    }
    // Last argument pointer.
    *(char **)(mem + sizeof(size_t) * argc) = NULL;
    // Update argv size.
    process->argv_size                      = required;
    process->argv                           = (char **)mem;

    return true;
}

// Load an executable and start a prepared process.
errno_t proc_start_raw(process_t *process) {
    // Claim the process for starting.
    if (!(atomic_fetch_and(&process->flags, ~PROC_PRESTART) & PROC_PRESTART)) {
        return -EINVAL;
    }

    mutex_acquire(&process->mtx, TIMESTAMP_US_MAX);

    // Load the executable.
    kbelf_dyn dyn = kbelf_dyn_create(process->pid);
    if (!dyn) {
        logkf(LOG_ERROR, "Out of memory to start %{cs}", process->binary);
        mutex_release(&process->mtx);
        return -EINVAL;
    }
    if (!kbelf_dyn_set_exec(dyn, process->binary, NULL)) {
        logkf(LOG_ERROR, "Failed to open %{cs}", process->binary);
        kbelf_dyn_destroy(dyn);
        mutex_release(&process->mtx);
        return -ENOENT;
    }
    if (!kbelf_dyn_load(dyn)) {
        kbelf_dyn_destroy(dyn);
        logkf(LOG_ERROR, "Failed to load %{cs}", process->binary);
        mutex_release(&process->mtx);
        return -ENOEXEC;
    }

    // Create the process' main thread.
    long thread = proc_create_thread_raw_unlocked(process, (size_t)kbelf_dyn_entrypoint(dyn), 0, SCHED_PRIO_NORMAL);
    if (thread < 0) {
        kbelf_dyn_unload(dyn);
        kbelf_dyn_destroy(dyn);
        mutex_release(&process->mtx);
        return thread;
    }
    atomic_store(&process->flags, PROC_RUNNING);
    mutex_release(&process->mtx);
    kbelf_dyn_destroy(dyn);
    logkf(LOG_INFO, "Process %{d} started", process->pid);

    return 0;
}


// Compare process thread entries by user thread ID.
int proc_thread_u_tid_cmp(void const *a_ptr, void const *b_ptr) {
    proc_thread_t const *a = a_ptr;
    proc_thread_t const *b = b_ptr;
    if (a->u_tid < b->u_tid) {
        return -1;
    } else if (a->u_tid > b->u_tid) {
        return 1;
    } else {
        return 0;
    }
}

// Create a new thread in a process.
// Returns created user thread handle or -errno.
static long proc_create_thread_raw_unlocked(process_t *process, size_t entry_point, size_t arg, int priority) {
    // Create an entry for a new thread.
    void *mem = realloc(process->threads, sizeof(tid_t) * (process->threads_len + 1));
    if (!mem) {
        return -ENOMEM;
    }
    process->threads = mem;

    // Create a thread.
    tid_t k_tid = thread_new_user(NULL, process, entry_point, arg, priority);
    if (k_tid < 0) {
        return k_tid;
    }
    sched_thread_t *thread = sched_get_thread(k_tid);

    thread->user_isr_ctx.mem_ctx   = &process->memmap.mem_ctx;
    thread->kernel_isr_ctx.mem_ctx = &process->memmap.mem_ctx;

    long   u_tid = 0;
    size_t i;
    for (i = 0; i < process->threads_len; i++) {
        if (u_tid == process->threads[i].u_tid) {
            u_tid++;
        }
    }

    // Add the thread to the list.
    proc_thread_t ent = {u_tid, k_tid};
    if (array_len_insert(&process->threads, sizeof(proc_thread_t), &process->threads_len, &ent, i)) {
        thread_resume(k_tid);
    } else {
        // TODO: Destroy thread.
        u_tid = -ENOMEM;
    }

    return u_tid;
}

// Create a new thread in a process.
// Returns created user thread handle or -errno.
long proc_create_thread_raw(process_t *process, size_t entry_point, size_t arg, int priority) {
    mutex_acquire(&process->mtx, TIMESTAMP_US_MAX);
    long res = proc_create_thread_raw_unlocked(process, entry_point, arg, priority);
    mutex_release(&process->mtx);
    return res;
}

// Get the corresponding kernel thread handle.
tid_t proc_get_thread_raw(process_t *process, long u_tid) {
    mutex_acquire_shared(&process->mtx, TIMESTAMP_US_MAX);

    proc_thread_t     to_find = {u_tid, 0};
    array_binsearch_t res =
        array_binsearch(process->threads, sizeof(proc_thread_t), process->threads_len, &to_find, proc_thread_u_tid_cmp);
    tid_t k_tid;
    if (res.found) {
        k_tid = process->threads[res.index].k_tid;
    } else {
        k_tid = -ENOENT;
    }

    mutex_release_shared(&process->mtx);

    return k_tid;
}


// Compares two `proc_fd_t` by user FD.
static int proc_fd_cmp_u(void const *a_ptr, void const *b_ptr) {
    proc_fd_t const *a = a_ptr;
    proc_fd_t const *b = b_ptr;
    if (a->u_fd < b->u_fd) {
        return -1;
    } else if (a->u_fd > b->u_fd) {
        return 1;
    } else {
        return 0;
    }
}

// Add a file to the process file handle list.
long proc_add_fd_raw(process_t *process, file_t k_fd) {
    mutex_acquire(&process->mtx, TIMESTAMP_US_MAX);
    proc_fd_t fd = {.k_fd = k_fd, .u_fd = 0};
    for (size_t i = 0; i < process->fds_len; i++) {
        if (process->fds[i].u_fd == fd.u_fd) {
            fd.u_fd++;
        } else if (process->fds[i].u_fd > fd.u_fd) {
            break;
        }
    }
    if (array_len_sorted_insert(&process->fds, sizeof(proc_fd_t), &process->fds_len, &fd, proc_fd_cmp_u)) {
        mutex_release(&process->mtx);
        return fd.u_fd;
    } else {
        mutex_release(&process->mtx);
        return -ENOMEM;
    }
}

// Find a file in the process file handle list.
file_t proc_find_fd_raw(process_t *process, long u_fd) {
    mutex_acquire_shared(&process->mtx, TIMESTAMP_US_MAX);
    for (size_t i = 0; i < process->fds_len; i++) {
        if (process->fds[i].u_fd == u_fd) {
            file_t k_fd = fs_file_clone(process->fds[i].k_fd);
            mutex_release_shared(&process->mtx);
            return k_fd;
        }
    }
    mutex_release_shared(&process->mtx);
    return FILE_NONE;
}

// Remove a file from the process file handle list.
errno_t proc_remove_fd_raw(process_t *process, long u_fd) {
    mutex_acquire(&process->mtx, TIMESTAMP_US_MAX);
    for (size_t i = 0; i < process->fds_len; i++) {
        if (process->fds[i].u_fd == u_fd) {
            file_t fd;
            array_len_remove(&process->fds, sizeof(proc_fd_t), &process->fds_len, &fd, i);
            fs_file_drop(fd);
            mutex_release(&process->mtx);
            return 0;
        }
    }
    mutex_release(&process->mtx);
    return -EBADF;
}


// Perform a pre-resume check for a user thread.
// Used to implement asynchronous events.
void proc_pre_resume_cb(sched_thread_t *thread) {
    process_t *const process = thread->process;
    if (proc_signals_pending_raw(process)) {
        logk_from_isr(LOG_DEBUG, "There be pending signals");
        sched_raise_from_isr(thread, false, proc_signal_handler);
    }
}

// Atomically check for pending signals.
bool proc_signals_pending_raw(process_t *process) {
    return atomic_load(&process->flags) & PROC_SIGPEND;
}

// Raise SIGKILL to a process.
static void proc_raise_sigkill_raw(process_t *process) {
    mutex_acquire(&process->mtx, TIMESTAMP_US_MAX);
    atomic_fetch_or(&process->flags, PROC_EXITING);
    process->state_code = W_SIGNALLED(SIGKILL);
    mutex_release(&process->mtx);

    // Add deleting runtime to the housekeeping list.
    assert_always(hk_add_once(0, clean_up_from_housekeeping, (void *)(long)process->pid) != -1);
}

// Raise a signal to a process' main thread or a specified thread, while suspending it's other threads.
errno_t proc_raise_signal_raw(process_t *process, int signum) {
    if (signum == SIGKILL) {
        proc_raise_sigkill_raw(process);
        return 0;
    }
    mutex_acquire(&process->mtx, TIMESTAMP_US_MAX);
    sigpending_t *node = malloc(sizeof(sigpending_t));
    if (!node) {
        return -ENOMEM;
    } else {
        node->signum = signum;
        dlist_append(&process->sigpending, &node->node);
        atomic_fetch_or(&process->flags, PROC_SIGPEND);
    }
    mutex_release(&process->mtx);
    return 0;
}



// Suspend all threads for a process except the current.
void proc_suspend(process_t *process, tid_t current) {
    mutex_acquire(&process->mtx, TIMESTAMP_US_MAX);
    for (size_t i = 0; i < process->threads_len; i++) {
        if (process->threads[i].k_tid != current) {
            thread_suspend(process->threads[i].k_tid, false);
        }
    }
    mutex_release(&process->mtx);
}

// Resume all threads for a process.
void proc_resume(process_t *process) {
    mutex_acquire(&process->mtx, TIMESTAMP_US_MAX);
    for (size_t i = 0; i < process->threads_len; i++) {
        thread_resume(process->threads[i].k_tid);
    }
    mutex_release(&process->mtx);
}

// Release all process runtime resources (threads, memory, files, etc.).
// Does not remove args, exit code, etc.
void proc_delete_runtime_raw(process_t *process) {
    // This may not be run from one of the process' threads because it kills all of them.
    for (size_t i = 0; i < process->threads_len; i++) {
        assert_dev_drop(sched_current_tid() != process->threads[i].k_tid);
    }

    if (process->pid == 1 && !allow_proc1_death()) {
        // Process 1 exited and now the kernel is dead.
        logk(LOG_FATAL, "Init process exited unexpectedly");
        panic_abort();
    }

    // Set the exiting flag so any return to user-mode kills the thread.
    mutex_acquire(&process->mtx, TIMESTAMP_US_MAX);
    if (atomic_load(&process->flags) & PROC_EXITED) {
        // Already exited, return now.
        mutex_release(&process->mtx);
        return;
    } else {
        // Flag the scheduler to start suspending threads.
        atomic_fetch_or(&process->flags, PROC_EXITING);
        atomic_thread_fence(memory_order_release);
    }

    // Destroy all threads.
    for (size_t i = 0; i < process->threads_len; i++) {
        thread_join(process->threads[i].k_tid);
    }
    process->threads_len = 0;
    free(process->threads);

    // Adopt all children to init.
    if (process->pid != 1) {
        process_t *init = procs[0];
        assert_dev_drop(init->pid == 1);
        dlist_concat(&init->children, &process->children);
    }

    //     // Unmap all memory regions.
    //     while (process->memmap.regions_len) {
    // #if !CONFIG_NOMMU
    //         proc_unmap_raw(process, process->memmap.regions[0].vaddr, process->memmap.regions[0].size);
    // #else
    //         proc_unmap_raw(process, process->memmap.regions[0].paddr, process->memmap.regions[0].size);
    // #endif
    //     }

    // Close pipes and files.
    for (size_t i = 0; i < process->fds_len; i++) {
        fs_file_drop(process->fds[i].k_fd);
    }
    process->fds_len = 0;
    free(process->fds);

    // Mark the process as exited.
    atomic_fetch_or(&process->flags, PROC_EXITED);
    atomic_fetch_and(&process->flags, ~PROC_EXITING & ~PROC_RUNNING);
    logkf(LOG_INFO, "Process %{d} stopped with code %{d}", process->pid, process->state_code);
    mutex_release(&process->mtx);
}



// Create a new, empty process.
pid_t proc_create(pid_t parent, char const *binary, int argc, char const *const *argv) {
    errno_procptr_t process = proc_create_raw(parent, binary, argc, argv);
    return process.errno < 0 ? process.errno : process.proc->pid;
}

// Delete a process and release any resources it had.
static bool proc_delete_impl(pid_t pid, bool only_prestart) {
    mutex_acquire(&proc_mtx, TIMESTAMP_US_MAX);
    process_t         dummy     = {.pid = pid};
    process_t        *dummy_ptr = &dummy;
    array_binsearch_t res       = array_binsearch(procs, sizeof(process_t *), procs_len, &dummy_ptr, proc_sort_pid_cmp);
    if (!res.found) {
        mutex_release(&proc_mtx);
        return false;
    }
    process_t *handle = procs[res.index];

    // Check for pre-start-ness.
    if (only_prestart && !(atomic_load(&handle->flags) & PROC_PRESTART)) {
        mutex_release(&proc_mtx);
        return false;
    }

    // Stop the possibly running process and release all run-time resources.
    proc_delete_runtime_raw(handle);

    // Remove from parent's child list.
    process_t *parent = handle->parent;
    if (parent) {
        mutex_acquire(&parent->mtx, TIMESTAMP_US_MAX);
        dlist_remove(&parent->children, &handle->node);
        mutex_release(&parent->mtx);
    }

    // Release kernel memory allocated to process.
    vmm_destroy_user_ctx(handle->memmap.mem_ctx);
    free(handle->argv);
    free(handle);
    array_lencap_remove(&procs, sizeof(process_t *), &procs_len, &procs_cap, NULL, res.index);
    mutex_release(&proc_mtx);

    return true;
}

// Delete a process only if it hasn't been started yet.
errno_t proc_delete_prestart(pid_t pid) {
    return proc_delete_impl(pid, true);
}

// Delete a process and release any resources it had.
errno_t proc_delete(pid_t pid) {
    return proc_delete_impl(pid, false);
}

// Get the process' flags.
errno64_t proc_getflags(pid_t pid) {
    mutex_acquire_shared(&proc_mtx, TIMESTAMP_US_MAX);
    process_t *proc = proc_get(pid);
    uint32_t   flags;
    if (proc) {
        flags = atomic_load(&proc->flags);
    } else {
        flags = -ENOENT;
    }
    mutex_release_shared(&proc_mtx);
    return flags;
}


// Load an executable and start a prepared process.
errno_t proc_start(pid_t pid) {
    mutex_acquire_shared(&proc_mtx, TIMESTAMP_US_MAX);
    process_t *proc = proc_get_unsafe(pid);
    errno_t    res;
    if (proc) {
        res = proc_start_raw(proc);
    } else {
        res = -ENOENT;
    }
    mutex_release_shared(&proc_mtx);
    return res;
}

// Check whether a process is a parent to another.
errno_bool_t proc_is_parent(pid_t parent, pid_t child) {
    mutex_acquire_shared(&proc_mtx, TIMESTAMP_US_MAX);
    process_t   *parent_proc = proc_get_unsafe(parent);
    process_t   *child_proc  = proc_get_unsafe(child);
    errno_bool_t res;
    if (!child_proc || !parent_proc) {
        res = -ENOENT;
    } else {
        res = child_proc->parent == parent_proc;
    }
    mutex_release_shared(&proc_mtx);
    return res;
}

// Raise a signal to a process' main thread, while suspending it's other threads.
errno_t proc_raise_signal(pid_t pid, int signum) {
    mutex_acquire_shared(&proc_mtx, TIMESTAMP_US_MAX);
    process_t   *proc = proc_get(pid);
    errno_bool_t res;
    if (proc) {
        res = proc_raise_signal_raw(proc, signum);
    } else {
        res = -ENOENT;
    }
    mutex_release_shared(&proc_mtx);
    return res;
}



// Determine string length in memory a user owns.
// Returns -1 if the user doesn't have access to any byte in the string.
ptrdiff_t strlen_from_user(pid_t pid, size_t user_vaddr, ptrdiff_t max_len) {
    mutex_acquire_shared(&proc_mtx, TIMESTAMP_US_MAX);
    ptrdiff_t res = strlen_from_user_raw(proc_get_unsafe(pid), user_vaddr, max_len);
    mutex_release_shared(&proc_mtx);
    return res;
}

// Copy bytes from user to kernel.
// Returns whether the user has access to all of these bytes.
// If the user doesn't have access, no copy is performed.
bool copy_from_user(pid_t pid, void *kernel_vaddr, size_t user_vaddr, size_t len) {
    mutex_acquire_shared(&proc_mtx, TIMESTAMP_US_MAX);
    bool res = copy_from_user_raw(proc_get_unsafe(pid), kernel_vaddr, user_vaddr, len);
    mutex_release_shared(&proc_mtx);
    return res;
}

// Copy from kernel to user.
// Returns whether the user has access to all of these bytes.
// If the user doesn't have access, no copy is performed.
bool copy_to_user(pid_t pid, size_t user_vaddr, void const *kernel_vaddr, size_t len) {
    mutex_acquire_shared(&proc_mtx, TIMESTAMP_US_MAX);
    bool res = copy_to_user_raw(proc_get_unsafe(pid), user_vaddr, kernel_vaddr, len);
    mutex_release_shared(&proc_mtx);
    return res;
}
