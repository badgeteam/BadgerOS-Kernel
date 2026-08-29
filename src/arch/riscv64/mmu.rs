use crate::{
    arch::mmu::ArchMMU,
    mem::{pmm::PAddrr, vmm::physmap::PTE},
};

use super::Riscv;

impl ArchMMU for Riscv {
    const BITS_PER_LEVEL: u32 = 9;

    fn enable_sum() {
        todo!()
    }

    fn disable_sum() {
        todo!()
    }

    fn check_sum() -> bool {
        todo!()
    }

    fn pack_pte(pte: PTE) -> usize {
        todo!()
    }

    fn unpack_pte(raw: usize, level: u8) -> PTE {
        todo!()
    }

    unsafe fn mmu_early_init() {
        todo!()
    }

    unsafe fn mmu_init(root: PAddrr) {
        todo!()
    }

    unsafe fn set_page_table(root: PAddrr, asid: u32) {
        todo!()
    }

    fn vmem_fence(vaddr: Option<usize>, asid: Option<u32>) {
        todo!()
    }
}
