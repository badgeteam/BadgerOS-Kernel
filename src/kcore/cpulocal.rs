// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::{sync::Arc, vec::Vec};

use crate::{
    cpu::{CpuFeatures, PhysCpuID, cpulocal::ArchCpuLocal},
    device::class::irqctl::IrqCtlDevice,
    kcore::sched::{Scheduler, Thread},
};

/// All CPU-local data.
/// The first four fields are placed near the start so that a small offset is sufficient to access them from assembly.
#[repr(C)]
#[derive(Default)]
pub struct CpuLocal {
    /// Architecture-specific CPU-local data.
    /// Must be the first member of this struct.
    pub arch: ArchCpuLocal,
    /// Current thread.
    pub thread: Option<Arc<Thread>>,
    /// What CPU ID this processor is.
    pub cpuid: PhysCpuID,
    /// What SMP index this CPU is.
    pub smp_index: u32,

    /// What features this CPU has.
    pub features: CpuFeatures,
    /// This CPU's scheduler.
    pub sched: Option<Scheduler>,
    /// External interrupt controllers attached to this CPU (LAPIC, PLIC contexts, etc.).
    pub ext_irqctls: Vec<Arc<dyn IrqCtlDevice>>,
}
