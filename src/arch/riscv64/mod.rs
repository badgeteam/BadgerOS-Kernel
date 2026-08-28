use super::ArchTrait;

pub struct Riscv;

impl ArchTrait for Riscv {
    const NAME: &'static str = "riscv64";
}
