// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

pub mod backtrace;
pub mod cpulocal;
#[cfg(feature = "dtb")]
pub mod dtb;
pub mod exception;
pub mod irq;
pub mod mmu;
pub mod panic;
mod sbi;
pub mod spinup;
pub mod thread;
pub mod timer;
pub mod usercopy;
pub mod usermode;

pub type CpuID = usize;

/// Detectable features that BadgerOS can run without but needs to support for userspace to use it.
#[derive(Default, Clone, Copy)]
pub struct CpuFeatures {
    pub f32: bool,
    pub f64: bool,
    pub vec: bool,
}
