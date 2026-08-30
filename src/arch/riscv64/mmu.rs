use core::arch::asm;

use crate::{
    arch::{mmu::ArchMMU, riscv64::csr},
    mem::{
        pmm::PAddrr,
        vmm::physmap::{ASID_BITS, PAGING_LEVELS, PTE},
    },
};

use super::Riscv;

static mut HAS_PBMT: bool = false;

#[inline(always)]
fn read_satp() -> usize {
    let res;
    unsafe { asm!("csrr {}, satp", out(reg)res, options(pure, nomem, nostack)) };
    res
}

impl ArchMMU for Riscv {
    const BITS_PER_LEVEL: u32 = 9;

    fn enable_sum() {
        unsafe {
            asm!("csrs sstatus, {mask}", mask = in(reg) csr::sstatus::SUM_MASK, options(nostack))
        };
    }

    fn disable_sum() {
        unsafe {
            asm!("csrc sstatus, {mask}", mask = in(reg) csr::sstatus::SUM_MASK, options(nostack))
        };
    }

    fn check_sum() -> bool {
        let mask: usize;
        unsafe { asm!("csrr {mask}, sstatus", mask=out(reg)mask, options(pure, nomem, nostack)) };
        mask & csr::sstatus::SUM_MASK != 0
    }

    fn pack_pte(pte: PTE) -> usize {
        let pbmt = if unsafe { HAS_PBMT } {
            ((pte.flags as usize >> 10) & 3) << 61
        } else {
            0
        };
        pbmt + (pte.ppn << 10) as usize + (pte.flags & 0b11_1111_1110) as usize + pte.valid as usize
    }

    fn unpack_pte(raw: usize, level: u8) -> PTE {
        PTE {
            ppn: (raw >> 10) % (1usize << 57),
            flags: ((raw & 0b11_1111_1110) + (((raw >> 61) & 0b11) << 10)) as u32,
            valid: raw & 1 != 0,
            leaf: raw & 0b1110 != 0,
            level,
        }
    }

    unsafe fn mmu_early_init() {
        unsafe {
            let satp: usize;
            asm!("csrr {r}, satp", r = out(reg) satp);
            let mode = satp >> 60;
            PAGING_LEVELS = mode as u32 - 8 + 3;
        }
    }

    unsafe fn mmu_init(root: PAddrr) {
        unsafe {
            // Set the kernel page table with the maximum ASID to detect how many ASID bits are available.
            Self::set_page_table(root, 0xffff);
            let asid = Self::get_asid();
            ASID_BITS = asid.trailing_ones();

            // Set kernel page table with ASID 0 this time (which is reserved for the kernel itself).
            Self::set_page_table(root, 0);

            // Virtual memory fence to ensure any new things in the kernel page table become visible.
            Self::vmem_fence(None, None);
        }
    }

    unsafe fn set_page_table(root: PAddrr, asid: u32) {
        let new_val = (root >> 12)
            + ((asid as usize) << 44)
            + (unsafe { PAGING_LEVELS as usize - 3 + 8 } << 60);
        unsafe { asm!("csrw satp, {new_val}", new_val = in(reg) new_val) };
    }

    fn get_page_table() -> PAddrr {
        (read_satp() & 0x00000fff_ffffffff) << 12
    }

    fn get_asid() -> u32 {
        (read_satp() >> 44) as u32 & 0xffff
    }

    #[inline(always)]
    fn vmem_fence(vaddr: Option<usize>, asid: Option<u32>) {
        unsafe {
            match (vaddr, asid) {
                (None, None) => asm!("sfence.vma"),
                (None, Some(asid)) => asm!("sfence.vma x0, {asid}", asid = in(reg) asid),
                (Some(vaddr), None) => asm!("sfence.vma {vaddr}, x0", vaddr = in(reg) vaddr),
                (Some(vaddr), Some(asid)) => {
                    asm!("sfence.vma {vaddr}, {asid}", vaddr = in(reg) vaddr, asid = in(reg) asid)
                }
            }
        }
    }
}
