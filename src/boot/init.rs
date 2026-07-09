// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{arch::asm, ffi::CStr};

use alloc::{boxed::Box, sync::Arc, vec::Vec};

use crate::{
    bindings::{
        log::{LogLevel, logk_unlocked},
        raw::{
            bootp_early_init, bootp_full_init, bootp_postheap_init, bootp_reclaim_mem,
            device_create_null_zero, kernel_heap_init, kmodule_t, limine_dtb_request,
        },
    },
    cpu::{self, spinup::arch_cpu_spinup},
    dev2::{
        self,
        bus::ata::{self, AtaBus},
        class::char::CharDevice,
    },
    filesystem::mount_root::mount_root_fs,
    kernel::{
        cpulocal::CpuLocal,
        sched::{Scheduler, Thread, thread_sleep, thread_yield},
        sync::mutex::Mutex,
    },
    ktest::{KTestWhen, ktests_runlevel},
    mem::{dma::DmaFromRef, vmm},
    process::{Process, usercopy::UserSlice},
    util::version,
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
        logkf_unlocked!(LogLevel::Info, "BadgerOS {}", version::RELEASE);
        logk_unlocked(LogLevel::Info, version::VERSION);
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
    ktests_runlevel(KTestWhen::Sched);

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

        dev2::registry::init();

        unsafe extern "C" {
            static bootp_dtb_req: limine_dtb_request;
        }
        dev2::dtb::init((*bootp_dtb_req.response).dtb_ptr as _);

        dev2::probe::start_thread();

        logkf!(LogLevel::Info, "Finished");

        loop {
            let _ = thread_sleep(1000000);

            logkf!(LogLevel::Debug, "Test ATA buses");
            for bus in dev2::registry::buses_by_type::<AtaBus>().unwrap() {
                logkf!(LogLevel::Debug, "Test ATA bus {}", &bus);

                let mut id = [0u16; 256];
                bus.ata_cmd(
                    ata::Command::IdentDev,
                    1 << 6,
                    0,
                    0,
                    0,
                    Some(DmaFromRef::from_mut(&mut id)),
                )
                .expect("ATA command failed");

                let supports_48bit = id[83] & (1 << 10) != 0;
                let block_size_exp;
                if id[106] & (1 << 14) == 0 {
                    block_size_exp = 9; // 512 bytes
                } else {
                    let block_size = id[117] as u64 + (id[118] as u64) << 16;
                    if block_size == 0 {
                        block_size_exp = 9; // 512 bytes
                    } else {
                        block_size_exp = block_size.trailing_zeros() as u8;
                    }
                }
                let block_count = (id[100] as u64)
                    + ((id[101] as u64) << 16)
                    + ((id[102] as u64) << 32)
                    + ((id[103] as u64) << 48);

                logkf!(
                    LogLevel::Debug,
                    "{}: 48-bit: {}; sec. size: {}; sec. count: {}",
                    &bus,
                    if supports_48bit { 'y' } else { 'n' },
                    1u64 << block_size_exp,
                    block_count
                );
            }
        }

        // After this is old device and init.
        return;

        device_create_null_zero();

        // Finish bootloader hand-over.
        bootp_full_init();
        // Scheduler is already running on BSP so we start the tick timer retroactively for it.
        cpu::timer::start_tick_timer();

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
        unsafe { bootp_reclaim_mem() };
    }
    ktests_runlevel(KTestWhen::RootFs);

    logkf!(LogLevel::Info, "Starting init process");
    Process::new_init().expect("Failed to start init process");
}
