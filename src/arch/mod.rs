#[cfg(target_arch = "riscv64")]
mod riscv64;
#[cfg(target_arch = "riscv64")]
pub type Arch = riscv64::Riscv;

pub mod except;
pub mod kcore;
pub mod misc;
pub mod mmu;
pub mod usermode;

/// Trait through which architecture code is implemented.
pub const trait ArchTrait:
    except::ArchExcept + kcore::ArchKCore + misc::ArchMisc + mmu::ArchMMU + usermode::ArchUsermode
{
    const NAME: &'static str;
}
