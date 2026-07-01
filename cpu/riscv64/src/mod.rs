// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::cmp::Ordering;

pub mod backtrace;
pub mod cpulocal;
#[cfg(feature = "dtb")]
pub mod dtb;
pub mod exception;
pub mod insn;
pub mod irq;
pub mod mmu;
pub mod panic;
mod sbi;
pub mod spinup;
pub mod thread;
pub mod timer;
pub mod usercopy;
pub mod usermode;

pub type PhysCpuID = usize;

/// Detectable features that BadgerOS can run without but needs to support for userspace to use it.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct CpuFeatures {
    pub f32: bool,
    pub f64: bool,
    pub vec: bool,
}

impl CpuFeatures {
    fn lt_impl(&self, other: &Self) -> bool {
        (!self.f32 && other.f32) || (!self.f64 && other.f64) || (!self.vec && other.vec)
    }
}

impl PartialOrd for CpuFeatures {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.lt_impl(other), other.lt_impl(self)) {
            (true, false) => Some(Ordering::Less),
            (false, false) => Some(Ordering::Equal),
            (true, true) => None,
            (false, true) => Some(Ordering::Greater),
        }
    }
}

pub const MACHINE_NAME: &'static str = "riscv64";
