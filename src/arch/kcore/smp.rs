use limine::mp::MpInfo;
use num::PrimInt;

use crate::arch::Arch;

pub trait ArchSmp {
    type CpuID: PrimInt;

    /// Do arch-specific CPU initialization.
    fn cpu_spinup();

    /// Load early architectural state and jump to [`crate::kcore::smp::limine_trampoline_2`].
    unsafe extern "C" fn limine_trampoline_1(info: &MpInfo) -> !;
}

pub type CpuID = <Arch as ArchSmp>::CpuID;
