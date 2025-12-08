
// SPDX-License-Identifier: MIT

#pragma once

#include "filesystem.h"
#include "list.h"
#include "mem/vmm.h"
#include "mutex.h"
#include "scheduler/scheduler.h"
#include "signal.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// A memory map entry.
typedef struct {
    // Base physical address of the region.
    size_t paddr;
#if !CONFIG_NOMMU
    // Base virtual address of the region.
    size_t vaddr;
#endif
    // Size of the region.
    size_t size;
    // Write permission.
    bool   write;
    // Execution permission.
    bool   exec;
} proc_memmap_ent_t;

// Process memory map information.
typedef struct proc_memmap_t {
    // Memory management context.
    vmm_ctx_t mem_ctx;
    // Number of mapped regions.
    size_t    regions_len;
#ifdef PROC_MEMMAP_MAX_REGIONS
    // Mapped regions.
    proc_memmap_ent_t regions[PROC_MEMMAP_MAX_REGIONS];
#else
    // Capacity for mapped regions.
    size_t             regions_cap;
    // Mapped regions.
    proc_memmap_ent_t *regions;
#endif
} proc_memmap_t;

// Process file descriptor.
typedef struct {
    long   u_fd;
    file_t k_fd;
} proc_fd_t;

// Process thread handle.
typedef struct {
    // ID by which the user code refers to the thread.
    long       u_tid;
    // Kernel thread handle.
    tid_t      k_tid;
    // Copy of thread exit code.
    int        exit_code;
    // Whether the thread was detached.
    atomic_int detached;
} proc_thread_t;

// Pending signal entry.
typedef struct {
    // Doubly-linked list node.
    dlist_node_t node;
    // Signal number.
    int          signum;
} sigpending_t;

// Globally unique process ID.
typedef int pid_t;

// A process and all of its resources.
typedef struct process_t {
    // Node for child process list.
    dlist_node_t   node;
    // Parent process, NULL for process 1.
    process_t     *parent;
    // Process binary.
    char const    *binary;
    // Number of arguments.
    int            argc;
    // Value of arguments.
    char         **argv;
    // Size required to store all of argv.
    size_t         argv_size;
    // Number of file descriptors.
    size_t         fds_len;
    // File descriptors.
    proc_fd_t     *fds;
    // Number of threads.
    size_t         threads_len;
    // Thread handles.
    proc_thread_t *threads;
    // Process ID.
    pid_t          pid;
    // Memory map information.
    proc_memmap_t  memmap;
    // Resource mutex used for multithreading processes.
    mutex_t        mtx;
    // Process status flags.
    atomic_int     flags;
    // Pending signals list.
    dlist_t        sigpending;
    // Child process list.
    dlist_t        children;
    // Signal handler virtual addresses.
    // First index is for signal handler returns.
    size_t         sighandlers[SIG_COUNT];
    // Exit code if applicable.
    int            state_code;
    // Total time usage.
    timeusage_t    timeusage;
} process_t;

// Indicates that a function returns a process or an -errno.
typedef struct {
    int        errno;
    process_t *proc;
} errno_procptr_t;
