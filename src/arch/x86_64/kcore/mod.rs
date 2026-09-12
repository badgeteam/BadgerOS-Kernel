use crate::arch::{kcore::ArchKCore, x86_64::X86_64};

pub mod cpulocal;
pub mod sched;
pub mod smp;
pub mod timer;

impl ArchKCore for X86_64 {}
