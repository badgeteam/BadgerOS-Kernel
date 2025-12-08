
// SPDX-License-Identifier: MIT

#include "assertions.h"
#include "bootp.h"
#include "cpulocal.h"
#include "errno.h"
#include "filesystem.h"
#include "housekeeping.h"
#include "interrupt.h"
#include "isr_ctx.h"
#include "kmodule.h"
#include "ktest.h"
#include "log.h"
#include "malloc.h"
#include "mem/vmm.h"
#include "panic.h"
#include "scheduler/scheduler.h"
#include "set.h"
#include "time.h"

#include <stdatomic.h>



// When set, a shutdown is initiated.
// 0: Do nothing.
// 1: Shut down (default).
// 2: Reboot.
atomic_int              kernel_shutdown_mode;
// Thread ID of kernel lifetime thread.
static tid_t            klifetime_tid;
// Set of threads that will be joined before kernel init comletes.
static set_t            kinit_join_threads = PTR_SET_EMPTY;
// Built-in kernel modules.
extern kmodule_t const *start_kmodules[] asm("__start_kmodules");
extern kmodule_t const *stop_kmodules[] asm("__stop_kmodules");

void        init_ramfs();
static void kernel_init();
static void userland_init();
static void userland_shutdown();
static void kernel_shutdown();
static void kernel_lifetime_func();



// CPU-local data of booting CPU.
static cpulocal_t bsp_cpulocal = {0};

// After control handover, the booting CPU core starts here and other cores wait.
// This sets up the basics of everything needed by the other systems of the kernel.
// When finished, the booting CPU will perform kernel initialization.
void basic_runtime_init() {
    isr_ctx_t tmp_ctx = {0};
    tmp_ctx.flags     = ISR_CTX_FLAG_KERNEL;
    tmp_ctx.cpulocal  = &bsp_cpulocal;

    // ISR initialization.
    irq_init(&tmp_ctx);
    // Early platform initialization.
    bootp_early_init();
    ktests_runlevel(KTEST_WHEN_EARLY);

    // Announce that we're alive.
    logk_from_isr(LOG_INFO, "================================================");
    logk_from_isr(LOG_INFO, "BadgerOS " CONFIGSTR_CPU " starting...");
    logk_from_isr(LOG_INFO, "================================================");

    // Kernel memory allocator initialization.
    kernel_heap_init();
    ktests_runlevel(KTEST_WHEN_HEAP);

    // Page alloc ready, so VMM can be initialized.
    vmm_init();
    // Post-heap protocol-dependent initialization.
    bootp_postheap_init();
    ktests_runlevel(KTEST_WHEN_VMM);

    // Global scheduler initialization.
    sched_init();
    sched_init_cpu(0);

    // Housekeeping thread initialization.
    hk_init();
    // Add the remainder of the kernel lifetime as a new thread.
    klifetime_tid = thread_new_kernel("main", (void *)kernel_lifetime_func, NULL, SCHED_PRIO_NORMAL);
    assert_always(klifetime_tid > 0);
    assert_always(thread_resume(klifetime_tid) >= 0);

    // Start the scheduler and enter the next phase in the kernel's lifetime.
    sched_exec();
    assert_unreachable();
}


// Manages the kernel's lifetime after basic runtime initialization.
static void kernel_lifetime_func() {
    ktests_runlevel(KTEST_WHEN_SCHED);

    // Initialize the built-in kernel modules.
    for (kmodule_t const **cur = start_kmodules; cur != stop_kmodules; cur++) {
        logkf(LOG_INFO, "Init built-in module '%{cs}'", (**cur).name);
        (**cur).init();
    }

    // Start the kernel services.
    kernel_init();
    // Start other CPUs.
    sched_start_altcpus();
    // After secondary CPUs are started, any potential reclaiming of bootloader memory is possible.
    bootp_reclaim_mem();
    // Start userland.
    userland_init();

    // The boot process is now complete, this thread will wait until a shutdown is issued.
    int shutdown_mode;
    do {
        thread_yield();
        shutdown_mode = atomic_load(&kernel_shutdown_mode);
    } while (shutdown_mode == 0);

    // Shut down the userland.
    userland_shutdown();
    // Tie up loose ends.
    kernel_shutdown();
    // Power off.
    if (kernel_shutdown_mode == 2) {
        logkf(LOG_INFO, "Restarting");
    } else {
        logkf(LOG_INFO, "Powering off");
    }
    while (1);
}

// Shutdown system call implementation.
void syscall_sys_shutdown(bool is_reboot) {
    logk(LOG_INFO, is_reboot ? "Reboot requested" : "Shutdown requested");
    atomic_store(&kernel_shutdown_mode, 1 + is_reboot);
}



// After basic runtime initialization, the booting CPU core continues here.
// This finishes the initialization of all kernel systems, resources and services.
// When finished, the non-booting CPUs will be started (method and entrypoints to be determined).
static void kernel_init() {
    // Full hardware initialization.
    bootp_full_init();

    // Wait for some final threads before mounting root FS.
    set_foreach(void, tid, &kinit_join_threads) {
        thread_join((tid_t)(size_t)tid);
    }
    set_clear(&kinit_join_threads);

    // Try to mount the root filesystem.
    fs_mount_root_fs();
    ktests_runlevel(KTEST_WHEN_ROOTFS);
}



// After kernel initialization, the booting CPU core continues here.
// This starts up the `init` process while other CPU cores wait for processes to be scheduled for them.
// When finished, this function returns and the thread should wait for a shutdown event.
static void userland_init() {
    logk(LOG_INFO, "Kernel initialized");
    // logk(LOG_INFO, "Starting init process");

    // char const *initbin = "/sbin/init";
    // pid_t       pid     = proc_create(-1, "/sbin/init", 1, &initbin);
    // assert_always(pid == 1);
    // assert_always(proc_start(pid) >= 0);
}



// When a shutdown event begins, exactly one CPU core runs this entire function.
// This signals all processes to exit (or be killed if they wait too long) and shuts down other CPU cores.
// When finished, the CPU continues to shut down the kernel.
static void userland_shutdown() {
    // if (proc_has_noninit()) {
    //     // Warn all processes of the imminent doom.
    //     logk(LOG_INFO, "Sending SIGHUP to running processes");
    //     proc_signal_all(SIGHUP);
    //     // Wait for one second to give them time.
    //     timestamp_us_t lim = time_us() + 1000000;
    //     while (time_us() < lim && proc_has_noninit()) thread_yield();

    //     if (proc_has_noninit()) {
    //         // Forcibly terminate all processes.
    //         logk(LOG_INFO, "Sending SIGKILL to running processes");
    //         proc_signal_all(SIGKILL);
    //     }
    // }

    // // Tell init it's now time to stop.
    // logk(LOG_INFO, "Sending SIGHUP to init");
    // proc_raise_signal(1, SIGHUP);
    // // Wait for a couple seconds to give it time.
    // timestamp_us_t lim = time_us() + 5000000;
    // while (time_us() < lim) {
    //     errno64_t flags = proc_getflags(1);
    //     if (flags == -ENOENT || (flags > 0 && !(flags & PROC_RUNNING))) {
    //         // When the init process stops, userland has successfully been shut down.
    //         return;
    //     }
    //     thread_yield();
    // }

    // // If init didn't stop by this point we're probably out of luck.
    // logk(LOG_FATAL, "Init process did not stop at shutdown");
    // panic_abort();
}



// When the userspace has been shut down, the CPU continues here.
// This will synchronize all filesystems and clean up any other resources not needed to finish hardware shutdown.
// When finished, the CPU continues to the platform-specific hardware shutdown / reboot handler.
static void kernel_shutdown() {
    // TODO: Filesystems flush.
}



// Add a thread to the list to block on for kernel init.
void klifetime_join_for_kinit(tid_t tid) {
    assert_dev_drop(sched_current_tid() == klifetime_tid);
    set_add(&kinit_join_threads, (void *)(size_t)tid);
}
