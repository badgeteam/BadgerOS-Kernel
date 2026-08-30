use super::ArchTrait;

pub struct Riscv;

pub mod csr;
pub mod except;
pub mod kcore;
pub mod misc;
pub mod mmu;
pub mod usermode;

impl ArchTrait for Riscv {
    const MACHINE: &'static str = "riscv64";
}
