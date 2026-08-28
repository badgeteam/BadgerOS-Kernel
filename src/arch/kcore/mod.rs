pub mod cpulocal;
pub mod sched;

pub trait ArchKCore: cpulocal::ArchCpuLocal + sched::ArchSched {}
