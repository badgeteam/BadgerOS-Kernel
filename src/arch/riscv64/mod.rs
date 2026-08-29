use super::ArchTrait;

pub struct Riscv;

pub mod csr;
pub mod except;
pub mod kcore;
pub mod misc;
pub mod mmu;
pub mod usermode;

impl ArchTrait for Riscv {
    const NAME: &'static str = "riscv64";
}
