use crate::arch::kcore::ArchKCore;

use super::Riscv;

pub mod cpulocal;
pub mod sched;

impl ArchKCore for Riscv {}
