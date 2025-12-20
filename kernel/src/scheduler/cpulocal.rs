// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::sync::Arc;

use crate::{
    cpu,
    scheduler::{Scheduler, Thread},
};

/// All CPU-local data.
#[repr(C)]
pub struct CpuLocal {
    /// Architecture-specific CPU-local data.
    pub arch: cpu::cpulocal::ArchCpuLocal,
    /// Current thread.
    pub thread: Option<Arc<Thread>>,
    /// This CPU's scheduler.
    pub sched: Scheduler,
}
