pub mod cpulocal;
pub mod sched;
pub mod smp;
pub mod timer;

pub trait ArchKCore:
    cpulocal::ArchCpuLocal + sched::ArchSched + smp::ArchSmp + timer::ArchTimer
{
}
