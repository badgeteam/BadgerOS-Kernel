#[cfg(target_arch = "riscv64")]
mod riscv64;
#[cfg(target_arch = "riscv64")]
pub type Arch = riscv64::Riscv;

pub mod except;
pub mod kcore;
pub mod usermode;

/// Trait through which architecture code is implemented.
pub const trait ArchTrait:
    kcore::ArchKCore + usermode::ArchUsermode + except::ArchExcept
{
    const NAME: &'static str;
}
