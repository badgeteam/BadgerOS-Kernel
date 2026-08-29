use crate::mem::{pmm::PAddrr, vmm::physmap::PTE};

pub trait ArchMMU {
    /// Virtual-address bits per page table level.
    const BITS_PER_LEVEL: u32;
    /// Value of invalid PTEs in page tables.
    const INVALID_PTE: usize = 0;

    /// Enable supervisor access to user memory.
    fn enable_sum();
    /// Disable supervisor access to user memory.
    fn disable_sum();
    /// Check supervisor access to user memory flag.
    fn check_sum() -> bool;

    /// Pack a PTE from generic representation.
    fn pack_pte(pte: PTE) -> usize;
    /// Unpack a PTE into generic representation.
    fn unpack_pte(raw: usize, level: u8) -> PTE;

    /// Prepare the MMU for later initialization and detect its features and mode.
    unsafe fn mmu_early_init();
    /// Initialize the MMU and switch kernel page tables.
    unsafe fn mmu_init(root: PAddrr);

    /// Update the active page table.
    unsafe fn set_page_table(root: PAddrr, asid: u32);
    /// Local TLB invalidation.
    fn vmem_fence(vaddr: Option<usize>, asid: Option<u32>);
}
