// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::c_int;

use alloc::{sync::Arc, vec::Vec};

use crate::{
    cpu::{CpuFeatures, PhysCpuID, cpulocal::ArchCpuLocal},
    device::class::irqctl::IrqCtlDevice,
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
    /// External interrupt controllers attached to this hart (e.g. PLIC contexts).
    /// Dispatched by the arch trap handler for external (and software) interrupts.
    pub ext_irqctls: Vec<Arc<dyn IrqCtlDevice>>,
}
