// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::mem::MaybeUninit;

use alloc::{sync::Arc, vec::Vec};

use crate::{
    LogLevel,
    arch::{
        Arch, ArchTrait,
        kcore::{cpulocal::ArchCpuLocal, smp::ArchSmp, timer::ArchTimer},
    },
    bindings::raw::kernel_heap_init,
    boot::protocol,
    device,
    filesystem::mount_root::mount_root_fs,
    kcore::{
        cpulocal::CpuLocal,
        sched::{Scheduler, Thread},
        sync::mutex::Mutex,
    },
    mem::vmm,
    misc::kmodule,
    process::Process,
    util::{
        ktest::{KTestWhen, ktests_runlevel},
        version,
    },
};

static mut BSP_CPULOCAL: MaybeUninit<CpuLocal> = MaybeUninit::uninit();

/// Sets up basic things like memory management and the scheduler.
/// Called by the entrypoint assembly code.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn basic_runtime_init() -> ! {
    unsafe {
        // Temporary CPU-local data in case an exception occurs before MM is up.
        BSP_CPULOCAL = MaybeUninit::new(CpuLocal::default());
        let bsp_cpulocal = (*&raw mut BSP_CPULOCAL).assume_init_mut();
        Arch::set_cpulocal(bsp_cpulocal);
        Arch::cpu_spinup();
        ktests_runlevel(KTestWhen::Early);

        // Early hand-over from bootloader to kernel.
        protocol::early_init();
        ktests_runlevel(KTestWhen::PMM);

        // Announce the kernel is alive.
        logkf_unlocked!(LogLevel::Info, "==============================");
        logkf_unlocked!(
            LogLevel::Info,
            "BadgerOS {} {}",
            Arch::MACHINE,
            version::RELEASE
        );
        logkf_unlocked!(LogLevel::Info, "{}", version::VERSION);
        logkf_unlocked!(LogLevel::Info, "==============================");

        // Set up memory management.
        kernel_heap_init();
        ktests_runlevel(KTestWhen::Heap);
        vmm::init();
        protocol::late_init();
        ktests_runlevel(KTestWhen::VMM);

        // Do the remainder of initialization with scheduler up.
        (*bsp_cpulocal).sched = Some(Scheduler::new().expect("Failed to prepare scheduler"));
        Thread::new(|| general_init(), None, Some("Kernel init".into()))
            .expect("Failed to prepare main init thread");
        (*bsp_cpulocal).sched.as_mut().unwrap().exec();
    }
}

/// Threads that will be joined before mounting the root filesystem and starting userland.
pub static INIT_BLOCK_THREADS: Mutex<Vec<Arc<Thread>>> = Mutex::new(Vec::new());

/// Main initialization function of the kernel.
/// Sets up most things after early boot.
unsafe fn general_init() {
    ktests_runlevel(KTestWhen::Sched);

    let smp_ok;
    unsafe {
        kmodule::init_builtins();

        device::init();

        // Scheduler is already running on BSP so we start the tick timer retroactively for it.
        Arch::start_tick_timer();
        Thread::new(|| loop {}, None, None);

        // Bring up APs.
        // smp_ok = match smp::poweron_all_aps() {
        //     Ok(_) => true,
        //     Err(x) => {
        //         logkf!(LogLevel::Error, "Failed to power on APs: {}", x);
        //         false
        //     }
        // };
        smp_ok = false;
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
        unsafe { protocol::reclaim_mem() };
    }
    ktests_runlevel(KTestWhen::RootFs);

    logkf!(LogLevel::Info, "Starting init process");
    Process::new_init().expect("Failed to start init process");
}
