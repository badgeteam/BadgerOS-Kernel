// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::{sync::Arc, vec::Vec};

use crate::{
    arch::{
        Arch,
        kcore::{cpulocal::ArchCpuLocal, smp::CpuID},
    },
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
    pub arch: <Arch as ArchCpuLocal>::CpuLocalData,
    /// Current thread.
    pub thread: Option<Arc<Thread>>,
    /// What CPU ID this processor is.
    pub cpuid: CpuID,
    /// What SMP index this CPU is.
    pub smp_index: u32,

    /// This CPU's scheduler.
    pub sched: Option<Scheduler>,
    /// External interrupt controllers attached to this CPU (LAPIC, PLIC contexts, etc.).
    pub ext_irqctls: Vec<Arc<dyn IrqCtlDevice>>,
}
