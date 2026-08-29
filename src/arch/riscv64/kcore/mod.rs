use crate::arch::kcore::ArchKCore;

use super::Riscv;

pub mod cpulocal;
pub mod sched;
pub mod smp;
pub mod timer;

impl ArchKCore for Riscv {}
