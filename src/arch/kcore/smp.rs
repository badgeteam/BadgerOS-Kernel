use num::PrimInt;

use crate::{arch::Arch, bindings::raw::limine_smp_info};

pub trait ArchSmp {
    type CpuID: PrimInt;

    /// Load early architectural state and jump to [`crate::kcore::smp::limine_trampoline_2`].
    unsafe extern "C" fn limine_trampoline_1(info: *mut limine_smp_info);
}

pub type CpuID = <Arch as ArchSmp>::CpuID;
