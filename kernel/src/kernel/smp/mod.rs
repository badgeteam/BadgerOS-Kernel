// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    mem::swap,
    ptr::{null_mut, slice_from_raw_parts_mut},
    sync::atomic::{Atomic, AtomicU32, AtomicUsize, Ordering},
};

use alloc::{boxed::Box, collections::btree_map::BTreeMap};

use crate::{
    bindings::{
        device::dtb::DtbNode,
        error::{EResult, Errno},
        log::LogLevel,
        raw::{limine_smp_info, limine_smp_request},
    },
    config,
    cpu::{
        self, CpuID,
        spinup::{arch_cpu_spinup, limine_trampoline_1},
    },
    kernel::{
        cpulocal::CpuLocal,
        sched::{Scheduler, thread_sleep, thread_yield},
        sync::mutex::Mutex,
    },
};

use super::sync::mutex::MutexGuard;

pub mod atomic_cpuset;
pub mod cpuset;

pub const CPU_SET_LEN: usize = config::MAX_CPUS.div_ceil(32) as usize;

/// Power status for a CPU.
#[repr(u32)]
enum PowerState {
    /// CPU is in a pre-bootloader hand-over state.
    PreHandover = 0,
    /// CPU is currently operational.
    Online = 1,
    /// CPU is in a low-power suspend state.
    Suspended = 2,
    /// CPU is fully powered off.
    PowerOff = 3,
}

impl From<u32> for PowerState {
    fn from(value: u32) -> Self {
        match value {
            0 => PowerState::PreHandover,
            1 => PowerState::Online,
            2 => PowerState::Suspended,
            3 => PowerState::PowerOff,
            _ => panic!("Invalid power state: {}", value),
        }
    }
}

struct SmpStatus {
    /// CPU-local data pointer.
    cpulocal: Box<CpuLocal>,
    /// Current power status as in [`PowerState`].
    power: AtomicU32,
}

struct SmpMaps {
    /// Map from SMP index to SMP status struct.
    by_index: BTreeMap<u32, SmpStatus>,
    /// Map from CPU ID to SMP index.
    by_cpuid: BTreeMap<CpuID, u32>,
    /// One more than the maximum allocated SMP index.
    cpu_index_end: u32,
}

static SMP_MAPS: Mutex<SmpMaps> = Mutex::new(SmpMaps {
    by_index: BTreeMap::new(),
    by_cpuid: BTreeMap::new(),
    cpu_index_end: 1,
});

#[unsafe(link_section = ".requests")]
#[unsafe(no_mangle)]
static mut SMP_REQ: limine_smp_request = limine_smp_request {
    id: [
        0xc7b1dd30df4c8b88,
        0x0a82e883a194f07b,
        0x95a67b819a1b857e,
        0xa0b61b723b6a73e0,
    ],
    revision: 3,
    response: null_mut(),
    flags: 0,
};

/// Initialize the SMP subsystem from DTB.
#[cfg(feature = "dtb")]
pub fn init_dtb(cpus_node: &DtbNode) {
    let bsp_cpuid: CpuID;
    unsafe {
        if SMP_REQ.response.is_null() {
            panic!("Missing Limine SMP response");
        }
        #[cfg(target_arch = "riscv64")]
        {
            bsp_cpuid = (*SMP_REQ.response).bsp_hartid as CpuID;
        }
        #[cfg(target_arch = "x86_64")]
        {
            bsp_cpuid = (*SMP_REQ.response).bsp_lapic_id as CpuID;
        }
    };

    let mut maps = SMP_MAPS.unintr_lock();
    let mut smp_counter = 1u32;
    for cpu in cpus_node.child_nodes() {
        let _ = try {
            let features = cpu::dtb::is_usable(cpu)?;
            let reg = cpu.get_prop("reg")?;
            let cpuid: CpuID = reg.read_uint() as CpuID;

            let smp_index: u32;
            let power;
            if cpuid == bsp_cpuid {
                smp_index = 0;
                power = PowerState::Online;
            } else {
                smp_index = smp_counter;
                smp_counter += 1;
                power = PowerState::PreHandover;
            }
            logkf!(
                LogLevel::Info,
                "Detected CPU{} (CPUID {})",
                smp_index,
                cpuid
            );

            let mut status = SmpStatus {
                cpulocal: Box::new(CpuLocal {
                    smp_index,
                    features,
                    ..Default::default()
                }),
                power: AtomicU32::new(power as u32),
            };

            status.cpulocal.smp_index = smp_index;
            status.cpulocal.cpuid = cpuid;

            maps.by_index.insert(smp_index, status);
            maps.by_cpuid.insert(cpuid, smp_index);
        };
    }

    maps.cpu_index_end = smp_counter;
    init_common(&mut maps);
}

/// Initialize the SMP subsystem.
fn init_common(maps: &mut SmpMaps) {
    unsafe {
        let new_cpulocal = &mut maps.by_index.get_mut(&0).unwrap().cpulocal;
        let old_cpulocal = &mut *CpuLocal::get();
        old_cpulocal.features = new_cpulocal.features;
        swap(new_cpulocal.as_mut(), old_cpulocal);
        CpuLocal::set(new_cpulocal.as_mut());
        c_api::smp_count = maps.by_cpuid.len() as i32;
    }
}

/// Power on another CPU from [`PowerState::PreHandover`].
fn poweron_from_prehandover<'a>(index: u32, mut maps: MutexGuard<'a, SmpMaps>) -> EResult<()> {
    let status = maps.by_index.get_mut(&index).unwrap();
    status.cpulocal.sched = Some(Scheduler::new()?);

    let smp_resp = unsafe { &*SMP_REQ.response };
    let cpus = unsafe {
        &mut *slice_from_raw_parts_mut(
            smp_resp.cpus as *mut &'static mut limine_smp_info,
            smp_resp.cpu_count as usize,
        )
    };

    logkf!(LogLevel::Info, "Powering on CPU{}", index);

    // find the correct CPU from the Limine MP response.
    #[cfg(target_arch = "riscv64")]
    let cpu = cpus
        .iter_mut()
        .find(|x| x.hartid == status.cpulocal.cpuid as _)
        .unwrap();
    #[cfg(target_arch = "x86_64")]
    let cpu = cpus
        .iter_mut()
        .find(|x| x.lapic_id == status.cpulocal.cpuid as _)
        .unwrap();

    cpu.extra_argument = status.cpulocal.as_mut() as *mut _ as _;
    let goto_addr = unsafe { &*((&raw const cpu.goto_address) as *const AtomicUsize) };
    goto_addr.store(limine_trampoline_1 as *const () as _, Ordering::Release);

    let maps = maps.demote();
    let status = maps.by_index.get(&index).ok_or(Errno::ENOENT)?;

    // No need to bother with waitlist because this will be fast anyway.
    while status.power.load(Ordering::Relaxed) != PowerState::Online as u32 {
        thread_yield();
    }

    Ok(())
}

/// Power on another CPU from [`PowerState::Suspended`].
fn poweron_from_suspended<'a>(index: u32, mut maps: MutexGuard<'a, SmpMaps>) -> EResult<()> {
    todo!()
}

/// Power on another CPU from [`PowerState::PowerOff`].
fn poweron_from_poweroff<'a>(index: u32, mut maps: MutexGuard<'a, SmpMaps>) -> EResult<()> {
    todo!()
}

/// Power on another CPU.
pub fn poweron(index: u32) -> EResult<()> {
    let mut maps = SMP_MAPS.unintr_lock();
    let status = maps.by_index.get_mut(&index).ok_or(Errno::ENOENT)?;
    let power: PowerState = status.power.load(Ordering::Relaxed).into();

    match power {
        PowerState::PreHandover => poweron_from_prehandover(index, maps),
        PowerState::Online => Ok(()),
        PowerState::Suspended => poweron_from_suspended(index, maps),
        PowerState::PowerOff => poweron_from_poweroff(index, maps),
    }
}

/// Sequentially power on all APs.
pub fn poweron_all_aps() -> EResult<()> {
    let end = SMP_MAPS.unintr_lock_shared().cpu_index_end;
    for smp_id in 1..end {
        match poweron(smp_id) {
            Ok(()) | Err(Errno::ENOENT) => (),
            Err(x) => return Err(x),
        }
    }
    Ok(())
}

/// Report that the current CPU is online.
pub fn report_online() {
    let maps = SMP_MAPS.unintr_lock_shared();
    let index = cur_cpu();
    if index != 0 {
        // CPU0 will report being online before the SMP maps are actually initialized.
        maps.by_index
            .get(&index)
            .as_deref()
            .unwrap()
            .power
            .store(PowerState::Online as u32, Ordering::Relaxed);
    }
    logkf!(LogLevel::Info, "CPU{} is now online", index);
}

/// Second stage trampoline for transferring control from Limine to BadgerOS.
pub unsafe extern "C" fn limine_trampoline_2(info: *mut limine_smp_info) -> ! {
    unsafe {
        let cpulocal = (*info).extra_argument as *mut CpuLocal;
        CpuLocal::set(cpulocal);
        arch_cpu_spinup();
        (*cpulocal).sched.as_mut().unwrap().exec();
    }
}

pub fn cur_cpu() -> u32 {
    unsafe { (*CpuLocal::get()).smp_index }
}

mod c_api {
    #[cfg(feature = "dtb")]
    use crate::bindings::raw::dtb_node_t;
    use crate::bindings::{
        device::{BaseDevice, DeviceFromRaw},
        raw::device_t,
    };

    use super::*;

    #[unsafe(no_mangle)]
    pub(super) static mut smp_count: i32 = 0;

    #[cfg(feature = "dtb")]
    #[unsafe(no_mangle)]
    unsafe extern "C" fn smp_init_dtb(node: *const dtb_node_t) {
        init_dtb(unsafe { &*(node as *const DtbNode) });
    }

    #[unsafe(no_mangle)]
    extern "C" fn smp_cur_cpu() -> u32 {
        unsafe { (*CpuLocal::get()).smp_index }
    }

    #[unsafe(no_mangle)]
    extern "C" fn smp_get_cpu(cpuid: usize) -> u32 {
        SMP_MAPS
            .unintr_lock()
            .by_cpuid
            .get(&(cpuid as CpuID))
            .cloned()
            .unwrap_or(u32::MAX)
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn cpulocal_set_irqctl(index: u32, device: *mut device_t) {
        let device = unsafe { BaseDevice::from_raw(device) };
        SMP_MAPS
            .unintr_lock()
            .by_index
            .get_mut(&index)
            .unwrap()
            .cpulocal
            .irqctl = Some(device);
    }
}
