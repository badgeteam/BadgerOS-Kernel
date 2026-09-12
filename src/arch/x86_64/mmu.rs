use core::arch::asm;

use crate::{
    arch::{
        mmu::ArchMMU,
        x86_64::{
            X86_64,
            cpuid::cpuid,
            msr::{self, efer},
        },
    },
    badgelib::irq::IrqGuard,
    mem::{
        pmm::PAddrr,
        vmm::{
            self,
            physmap::{self, ASID_BITS, PAGING_LEVELS, PTE},
        },
    },
};

pub mod cr4 {
    // Prevent kernel data access to user memory.
    pub const SMAP: usize = 1 << 21;
    // Prevent kernel execution of user memory.
    pub const SMEP: usize = 1 << 20;
    // Enable PCIDs.
    pub const PCIDE: usize = 1 << 17;
    // Enable global pages.
    pub const PGE: usize = 1 << 7;
}

pub mod pte {
    // Non-executable page.
    pub const NX: usize = 1 << 63;
    // Global page.
    pub const G: usize = 1 << 8;
}

// TODO: Supporting INVLPGB and TLBSYNC needs further support in the VMM subsystem.

static mut SMAP: bool = false; // Supports CR4.SMAP.
static mut SMEP: bool = false; // Supports CR4.SMEP.
static mut INVPCID: bool = false; // Supports the INVPCID instruction.
static mut INVLPGA: bool = false; // Supports the INVLPGA instruction.
static mut PCID: bool = false; // Supports PCIDs.
static mut PTE_NX: bool = false; // Supports the NX bit in PTEs.
static mut PTE_G: bool = false; // Supports global pages.

#[inline(always)]
fn read_cr3() -> usize {
    unsafe {
        let cr3;
        asm!("mov {}, cr3", out(reg)cr3, options(nomem, nostack, preserves_flags));
        cr3
    }
}

unsafe fn write_cr3(cr3: usize) {
    unsafe {
        asm!("mov cr3, {}", in(reg)cr3, options(nostack, preserves_flags));
    }
}

// Use the most pessimistic fallbacks here.
static mut FLUSH_VADDR_PCID: fn(usize, u16) = do_invlpg as _;
static mut FLUSH_VADDR: fn(usize, u16) = do_invlpg as _;
static mut FLUSH_PCID: fn(usize, u16) = do_cr3_flush as _;
static mut FLUSH: fn(usize, u16) = do_cr3_flush as _;

// Invalidate a virtual address in all PCIDs with INVLPG.
fn do_invlpg(vaddr: usize, _pcid: u16) {
    unsafe {
        asm!("invlpg [{}]", in(reg)vaddr);
    }
}

// Invalidate a virtual address in a specific PCID with INVLPGA.
fn do_invlpga(vaddr: usize, pcid: u16) {
    unsafe {
        asm!("invlpga", in("rax")vaddr, in("ecx")pcid);
    }
}

// Invalidate a virtual address in a specific PCID with INVPCID.
fn do_invpcid_vma(vaddr: usize, pcid: u16) {
    unsafe {
        let spec = [vaddr as u64, pcid as u64];
        asm!("invpcid {:r}, [{}]", in(reg)0, in(reg)&spec);
    }
}

// Invalidate an entire PCID with INVPCID.
fn do_invpcid(_vaddr: usize, pcid: u16) {
    unsafe {
        let spec = [0u64, pcid as u64];
        asm!("invpcid {:r}, [{}]", in(reg)1, in(reg)&spec);
    }
}

// Invalidate the entire TLB with INVPCID.
fn do_invpcid_flush(_vaddr: usize, _pcid: u16) {
    unsafe {
        let spec = [0u64; 2];
        asm!("invpcid {:r}, [{}]", in(reg)2, in(reg)&spec);
    }
}

// Invalidate the entire TLB by toggling off CR4.PGE (PCID=1).
fn do_pge_flush(_vaddr: usize, _pcid: u16) {
    unsafe {
        let _noirq = IrqGuard::new();
        let mut cr4: usize;
        asm!("mov {}, cr4", out(reg)cr4);
        cr4 &= !cr4::PGE;
        asm!("mov cr4, {}", in(reg)cr4);
        cr4 |= cr4::PGE;
        asm!("mov cr4, {}", in(reg)cr4);
    }
}

// Invalidate the entire TLB by reading then writing CR3 (PCID=0).
fn do_cr3_flush(_vaddr: usize, _pcid: u16) {
    unsafe {
        write_cr3(read_cr3());
    }
}

impl ArchMMU for X86_64 {
    const BITS_PER_LEVEL: u32 = 9;

    #[inline(always)]
    fn enable_sum() {
        unsafe {
            if SMAP {
                // The AC flag is not covered under the preserves_flags option.
                asm!("stac", options(nostack, preserves_flags));
            }
        }
    }

    #[inline(always)]
    fn disable_sum() {
        unsafe {
            if SMAP {
                // The AC flag is not covered under the preserves_flags option.
                asm!("clac", options(nostack, preserves_flags));
            }
        }
    }

    fn check_sum() -> bool {
        unsafe {
            if !SMAP {
                return true;
            }
            let rflags: usize;
            asm!("pushf; pop {}", out(reg)rflags);
            (rflags & (1 << 18)) != 0
        }
    }

    fn pack_pte(pte: PTE) -> usize {
        let mut packed = 0;

        if unsafe { PTE_G } && (pte.flags & physmap::flags::G) != 0 {
            packed |= pte::G;
        }

        packed
    }

    fn unpack_pte(raw: usize, level: u8) -> PTE {
        todo!()
    }

    unsafe fn mmu_early_init() {
        unsafe {
            let ext_id_stepping = cpuid(0x8000_0001);
            INVLPGA = (ext_id_stepping.ecx & (1 << 2)) != 0;
            PTE_NX = (ext_id_stepping.edx & (1 << 20)) != 0;

            let id_stepping = cpuid(0x0000_0001);
            PCID = (id_stepping.ecx & (1 << 13)) != 0;
            PTE_G = (id_stepping.edx & (1 << 13)) != 0;

            let id_flags = cpuid(0x0000_0007);
            SMAP = (id_flags.ebx & (1 << 20)) != 0;
            SMEP = (id_flags.ebx & (1 << 7)) != 0;
            INVPCID = (id_flags.ebx & (1 << 10)) != 0;

            let cr4: u64;
            asm!("mov {}, cr4", out(reg)cr4);
            let la57 = (cr4 & (1 << 12)) != 0;

            ASID_BITS = if PCID { 12 } else { 0 };
            PAGING_LEVELS = if la57 { 5 } else { 4 };

            // Choose desired TLB flush scheme.
            if PCID && PTE_G {
                FLUSH_PCID = do_pge_flush as _;
                FLUSH = do_pge_flush as _;
            }
            if INVPCID {
                FLUSH_VADDR_PCID = do_invpcid_vma as _;
                FLUSH_PCID = do_invpcid as _;
                FLUSH = do_invpcid_flush as _;
            }
        }
    }

    unsafe fn mmu_init(root: PAddrr) {
        unsafe {
            Self::set_page_table(root, 0);

            let mut cr4: usize;
            asm!("mov {}, cr4", out(reg)cr4);
            cr4 |= PTE_G as usize * cr4::PGE;
            cr4 |= PCID as usize * cr4::PCIDE;
            cr4 |= SMAP as usize * cr4::SMAP;
            cr4 |= SMEP as usize * cr4::SMEP;
            asm!("mov cr4, {}", in(reg)cr4);

            let mut efer = msr::read(efer::ADDR);
            efer |= PTE_NX as u64 * efer::NXE_MASK;
            msr::write(efer::ADDR, efer);

            Self::vmem_fence(None, None);
        }
    }

    unsafe fn set_page_table(root: PAddrr, asid: u32) {
        unsafe {
            if !PCID {
                debug_assert!(asid == 0);
            } else {
                debug_assert!(asid <= 0x1000);
            }
            write_cr3(root | (asid as usize & 0xfff));
        }
    }

    fn get_page_table() -> PAddrr {
        read_cr3() & 0x0007_ffff_ffff_f000
    }

    fn get_asid() -> u32 {
        (read_cr3() & 0x0fff) as u32
    }

    #[inline(always)]
    fn vmem_fence(vaddr: Option<usize>, asid: Option<u32>) {
        unsafe {
            match (vaddr, asid) {
                (Some(vaddr), Some(asid)) => FLUSH_VADDR_PCID(vaddr, asid as u16),
                (Some(vaddr), None) => FLUSH_VADDR(vaddr, 0),
                (None, Some(asid)) => FLUSH_PCID(0, asid as u16),
                (None, None) => FLUSH(0, 0),
            }
        }
    }
}
