// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    mem::swap,
    sync::atomic::{AtomicU32, Ordering},
};

use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc};

#[cfg(feature = "dtb")]
use dtb;
use limine::{mp::MpInfo, request::MpRequest};

#[cfg(feature = "acpi")]
use crate::device::acpi::table::{MadtEntries, MadtEntry};
use crate::{
    LogLevel,
    arch::{
        Arch,
        kcore::{
            cpulocal::ArchCpuLocal,
            smp::{ArchSmp, CpuID},
        },
    },
    device::class::irqctl::IrqCtlDevice,
    error::{EResult, Errno},
    kcore::{
        cpulocal::CpuLocal,
        sched::{Scheduler, thread_yield},
        sync::mutex::{Mutex, SharedMutexGuard},
    },
};

use super::sync::mutex::MutexGuard;

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
static SMP_REQ: MpRequest = MpRequest::new(0);

/// Initialize the SMP subsystem from DTB.
#[cfg(feature = "dtb")]
pub fn init_dtb(cpus_node: &dtb::DtbNode) {
    let smp_req = SMP_REQ.response().expect("Missing SMP response");
    #[cfg(target_arch = "riscv64")]
    let bsp_cpuid = smp_req.bsp_hartid as CpuID;
    #[cfg(target_arch = "x86_64")]
    let bsp_cpuid = smp_req.bsp_lapic_id as CpuID;

    let mut maps = SMP_MAPS.unintr_lock();
    let mut smp_counter = 1u32;
    for cpu in cpus_node.nodes.values() {
        let _ = try {
            // TODO: Check for usability.
            // let features = crate::cpu::dtb::is_usable(cpu)?;
            let cpuid: CpuID = cpu.prop_uint("reg")? as CpuID;

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

#[cfg(feature = "acpi")]
pub fn init_acpi(madt: MadtEntries<'_>) {
    let smp_req = SMP_REQ.response().expect("Missing SMP response");
    #[cfg(target_arch = "riscv64")]
    let bsp_cpuid = smp_req.bsp_hartid as CpuID;
    #[cfg(target_arch = "x86_64")]
    let bsp_cpuid = smp_req.bsp_lapic_id as CpuID;

    let mut maps = SMP_MAPS.unintr_lock();
    let mut smp_counter = 1u32;
    let mut add_cpu = |cpuid: CpuID| {
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
                ..Default::default()
            }),
            power: AtomicU32::new(power as u32),
        };

        status.cpulocal.smp_index = smp_index;
        status.cpulocal.cpuid = cpuid;

        maps.by_index.insert(smp_index, status);
        maps.by_cpuid.insert(cpuid, smp_index);
    };

    // For SMP init we care specifically about the LAPICs.
    for entry in madt {
        use MadtEntry::*;
        match entry {
            Lapic(lapic) => {
                if lapic.flags & 3 != 0 {
                    // Either already online, or online capable.
                    add_cpu(lapic.id as CpuID)
                }
            }
            LocalX2apic(lapic) => {
                if lapic.flags & 3 != 0 {
                    // Either already online, or online capable.
                    add_cpu(lapic.id as CpuID)
                }
            }
            _ => (),
        }
    }

    maps.cpu_index_end = smp_counter;
    init_common(&mut maps);
}

/// Get SMP index from physical CPU ID.
pub fn by_phys_id(cpuid: CpuID) -> Option<u32> {
    SMP_MAPS.unintr_lock().by_cpuid.get(&cpuid).cloned()
}

/// Attach an external interrupt controller (e.g. a PLIC context) to a hart.
/// The arch trap handler dispatches external interrupts to all controllers registered here.
///
/// Must be called before the target hart is taking external interrupts from this controller.
pub fn register_ext_irqctl(smp_index: u32, ctl: Arc<dyn IrqCtlDevice>) -> EResult<()> {
    let mut maps = SMP_MAPS.unintr_lock();
    let status = maps.by_index.get_mut(&smp_index).ok_or(Errno::ENOENT)?;
    status.cpulocal.ext_irqctls.try_reserve(1)?;
    status.cpulocal.ext_irqctls.push(ctl);
    Ok(())
}

/// Initialize the SMP subsystem.
fn init_common(maps: &mut SmpMaps) {
    unsafe {
        let new_cpulocal = &mut maps.by_index.get_mut(&0).unwrap().cpulocal;
        let old_cpulocal = &mut *Arch::get_cpulocal();
        swap(new_cpulocal.as_mut(), old_cpulocal);
        Arch::set_cpulocal(new_cpulocal.as_mut());
    }
}

/// Power on another CPU from [`PowerState::PreHandover`].
fn poweron_from_prehandover<'a>(index: u32, mut maps: MutexGuard<'a, SmpMaps>) -> EResult<()> {
    let status = maps.by_index.get_mut(&index).unwrap();
    status.cpulocal.sched = Some(Scheduler::new()?);

    let smp_resp = SMP_REQ.response().unwrap();
    let cpus = smp_resp.cpus();

    logkf!(LogLevel::Info, "Powering on CPU{}", index);

    // find the correct CPU from the Limine MP response.
    #[cfg(target_arch = "riscv64")]
    let cpu = cpus
        .iter()
        .find(|x| x.hartid == status.cpulocal.cpuid as _)
        .unwrap();
    #[cfg(target_arch = "x86_64")]
    let cpu = cpus
        .iter()
        .find(|x| x.lapic_id == status.cpulocal.cpuid as _)
        .unwrap();

    cpu.bootstrap(
        Arch::limine_trampoline_1,
        status.cpulocal.as_mut() as *mut _ as _,
    );

    let maps = maps.demote();
    let status = maps.by_index.get(&index).ok_or(Errno::ENOENT)?;

    // No need to bother with waitlist because this will be fast anyway.
    while status.power.load(Ordering::Relaxed) != PowerState::Online as u32 {
        thread_yield();
    }

    Ok(())
}

/// Power on another CPU from [`PowerState::Suspended`].
fn poweron_from_suspended<'a>(_index: u32, _maps: MutexGuard<'a, SmpMaps>) -> EResult<()> {
    todo!()
}

/// Power on another CPU from [`PowerState::PowerOff`].
fn poweron_from_poweroff<'a>(_index: u32, _maps: MutexGuard<'a, SmpMaps>) -> EResult<()> {
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
pub unsafe extern "C" fn limine_trampoline_2(info: &MpInfo) -> ! {
    unsafe {
        let cpulocal = info.extra_argument() as *mut CpuLocal;
        Arch::set_cpulocal(cpulocal);
        Arch::cpu_spinup();
        (*cpulocal).sched.as_mut().unwrap().exec();
    }
}

/// One more than the maximum allocated SMP index.
pub fn cpu_index_end() -> u32 {
    SMP_MAPS.unintr_lock_shared().cpu_index_end
}

/// Get scheduler for some CPU.
pub fn get_sched_for(cpu: u32) -> Option<SharedMutexGuard<'static, Scheduler>> {
    let maps = SMP_MAPS.unintr_lock_shared();
    if !maps.by_index.contains_key(&cpu) {
        return None;
    }
    maps.try_convert(|x| try { x.by_index.get(&cpu)?.cpulocal.sched.as_ref()? })
}

pub fn cur_cpu() -> u32 {
    unsafe { (*Arch::get_cpulocal()).smp_index }
}
