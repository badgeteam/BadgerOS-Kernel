// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{mem::swap, ptr::null_mut};

use alloc::{boxed::Box, collections::btree_map::BTreeMap};

use crate::{
    bindings::{
        device::dtb::DtbNode,
        error::EResult,
        log::LogLevel,
        raw::{limine_smp_info, limine_smp_request},
    },
    cpu::{self, CpuID, spinup::arch_cpu_spinup},
    kernel::{cpulocal::CpuLocal, sync::mutex::Mutex},
};

struct SmpStatus {
    /// CPU-local data pointer.
    cpulocal: Box<CpuLocal>,
}

struct SmpMaps {
    /// Map from SMP index to SMP status struct.
    by_index: BTreeMap<u32, SmpStatus>,
    /// Map from CPU ID to SMP index.
    by_cpuid: BTreeMap<CpuID, u32>,
}

static SMP_MAPS: Mutex<SmpMaps> = Mutex::new(SmpMaps {
    by_index: BTreeMap::new(),
    by_cpuid: BTreeMap::new(),
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
        bsp_cpuid = (*SMP_REQ.response).bsp_hartid as CpuID;
    };

    let mut maps = SMP_MAPS.unintr_lock();
    let mut smp_counter = 1u32;
    for cpu in cpus_node.child_nodes() {
        let _ = try {
            let features = cpu::dtb::is_usable(cpu)?;
            let reg = cpu.get_prop("reg")?;
            let cpuid: CpuID = reg.read_uint() as CpuID;

            let smp_index: u32;
            if cpuid == bsp_cpuid {
                smp_index = 0;
            } else {
                smp_index = smp_counter;
                smp_counter += 1;
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
            };

            status.cpulocal.smp_index = smp_index;
            #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
            {
                status.cpulocal.arch.hartid = cpuid;
            }

            maps.by_index.insert(smp_index, status);
            maps.by_cpuid.insert(cpuid, smp_index);
        };
    }

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

/// Power on another CPU.
pub fn poweron(cpu: u32) -> EResult<()> {
    todo!()
}

/// Second stage trampoline for transferring control from Limine to BadgerOS.
#[unsafe(no_mangle)]
unsafe extern "C" fn limine_trampoline_2(info: *mut limine_smp_info) -> ! {
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
