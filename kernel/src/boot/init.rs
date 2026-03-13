// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::CStr;

use alloc::{boxed::Box, sync::Arc, vec::Vec};

use crate::{
    bindings::{
        log::{LogLevel, logk_unlocked},
        raw::{
            bootp_early_init, bootp_full_init, bootp_postheap_init, bootp_reclaim_mem,
            kernel_heap_init, kmodule_t, rawputc,
        },
    },
    cpu::{self, spinup::arch_cpu_spinup},
    filesystem::{self, mount_root::mount_root_fs},
    kernel::{
        cpulocal::CpuLocal,
        sched::{Scheduler, Thread},
        smp,
        sync::mutex::Mutex,
    },
    ktest::{KTestWhen, ktests_runlevel},
    mem::vmm,
    process::Process,
};

unsafe extern "C" {
    static __start_kmodules: *const kmodule_t;
    static __stop_kmodules: *const kmodule_t;
}

/// Sets up basic things like memory management and the scheduler.
/// Called by the entrypoint assembly code.
#[unsafe(no_mangle)]
unsafe extern "C" fn basic_runtime_init() -> ! {
    unsafe {
        // Temporary CPU-local data in case an exception occurs before MM is up.
        let mut tmp_cpulocal = CpuLocal::default();
        CpuLocal::set(&raw mut tmp_cpulocal);
        arch_cpu_spinup();

        // Early hand-over from bootloader to kernel.
        bootp_early_init();
        ktests_runlevel(KTestWhen::Early);

        // Announce the kernel is alive.
        logk_unlocked(LogLevel::Info, "==============================");
        #[cfg(target_arch = "riscv32")]
        let arch = "riscv32";
        #[cfg(target_arch = "riscv64")]
        let arch = "riscv64";
        #[cfg(target_arch = "x86_64")]
        let arch = "x86_64";
        logkf_unlocked!(LogLevel::Info, "BadgerOS {} starting", arch);
        logk_unlocked(LogLevel::Info, "==============================");

        // Set up memory management.
        kernel_heap_init();
        ktests_runlevel(KTestWhen::Heap);
        vmm::init();
        bootp_postheap_init();
        ktests_runlevel(KTestWhen::VMM);

        // Move the CPU-local data onto the heap.
        let cpulocal = Box::into_raw(Box::new(tmp_cpulocal));
        CpuLocal::set(cpulocal);

        // Do the remainder of initialization with scheduler up.
        (*cpulocal).sched = Some(Scheduler::new().expect("Failed to prepare scheduler"));
        Thread::new(|| general_init(), None, Some("Kernel init".into()))
            .expect("Failed to prepare main init thread");
        (*cpulocal).sched.as_mut().unwrap().exec();
    }
}

/// Threads that will be joined before mounting the root filesystem and starting userland.
pub static INIT_BLOCK_THREADS: Mutex<Vec<Arc<Thread>>> = Mutex::new(Vec::new());

/// Main initialization function of the kernel.
/// Sets up most things after early boot.
unsafe fn general_init() {
    let smp_ok;
    unsafe {
        let mut cur = &raw const __start_kmodules;
        while cur != &raw const __stop_kmodules {
            logkf!(
                LogLevel::Info,
                "Init build-in module '{}'",
                CStr::from_ptr((**cur).name).to_str().unwrap()
            );
            if let Some(init) = (**cur).init {
                init();
            }
            cur = cur.add(1);
        }

        // Finish bootloader hand-over.
        bootp_full_init();
        // Scheduler is already running on BSP so we start the tick timer retroactively for it.
        cpu::timer::start_tick_timer();

        // Bring up APs.
        smp_ok = match smp::poweron_all_aps() {
            Ok(_) => true,
            Err(x) => {
                logkf!(LogLevel::Error, "Failed to power on APs: {}", x);
                false
            }
        };
    }

    let init_block_threads = &mut *INIT_BLOCK_THREADS.unintr_lock();
    for thread in init_block_threads {
        thread.join().unwrap();
    }
    logkf!(LogLevel::Info, "Kernel initialized");

    mount_root_fs();
    if smp_ok {
        // We have now definitely stopped using all memory in bootloader reclaimable regions.
        // Exit the bootloader's services and reclaim all reclaimable memory.
        unsafe { bootp_reclaim_mem() };
    }

    logkf!(LogLevel::Info, "Starting init process");
    Process::new_init().expect("Failed to start init process");
}
