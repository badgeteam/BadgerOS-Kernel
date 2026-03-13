// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::c_int;

use alloc::sync::Arc;

use crate::{
    bindings::{
        device::{BaseDevice, DeviceFromRaw},
        raw::device_t,
    },
    cpu::{CpuFeatures, PhysCpuID, cpulocal::ArchCpuLocal},
    kernel::sched::{Scheduler, Thread},
};

/// All CPU-local data.
#[repr(C)]
#[derive(Default)]
pub struct CpuLocal {
    /// Architecture-specific CPU-local data.
    /// Must be the first member of this struct.
    pub arch: ArchCpuLocal,
    /// What CPU ID this processor is.
    pub cpuid: PhysCpuID,
    /// What SMP index this CPU is.
    pub smp_index: u32,
    /// What features this CPU has.
    pub features: CpuFeatures,
    /// Current thread.
    pub thread: Option<Arc<Thread>>,
    /// This CPU's scheduler.
    pub sched: Option<Scheduler>,
    /// The CPU's direct interrupt controller.
    pub irqctl: Option<BaseDevice>,
}

unsafe extern "C" fn cpulocal_set_irqctl(_smp_idx: c_int, irqctl: *mut device_t) {
    unsafe {
        (*CpuLocal::get()).irqctl = Some(BaseDevice::from_raw(irqctl));
    }
}
