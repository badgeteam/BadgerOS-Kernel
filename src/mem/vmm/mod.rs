// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{mem::MaybeUninit, ops::Deref, ptr::null};

use alloc::sync::Arc;
use map::{KernelVmSpace, Mapping};
use memobject::{MappablePage, RawMemory};

use crate::{
    bindings::log::LogLevel,
    config::PAGE_SIZE,
    cpu::{mmu, usercopy::fallible_store_u8},
    mem::pmm::{self, PAddrr},
};

mod c_api;
pub mod map;
pub mod memobject;
pub mod pagecache;
pub mod physmap;
pub mod vmfence;

/// Mapping protection flags.
pub mod prot {
    use crate::cpu::mmu;

    /// Mapping is readable.
    pub const READ: u8 = 1 << 0;
    /// Mapping is writable.
    pub const WRITE: u8 = 1 << 1;
    /// Mapping is executable.
    pub const EXEC: u8 = 1 << 2;
    /// Mapping is non-cacheable, idempotent, weakly-ordered (e.g. framebuffer memory).
    pub const NC: u8 = 1 << 3;
    /// Mapping is non-cacheable, non-idempotent, strongly-ordered (e.g. memory-mapped I/O).
    pub const IO: u8 = 1 << 4;

    /// Convert MMU flags into prot flags.
    pub(super) const fn from_mmu_flags(mmu_flags: u32) -> u8 {
        let mut prot = 0;
        if mmu_flags & mmu::flags::R != 0 {
            prot |= READ;
        }
        if mmu_flags & mmu::flags::W != 0 {
            prot |= WRITE;
        }
        if mmu_flags & mmu::flags::X != 0 {
            prot |= EXEC;
        }
        if mmu_flags & mmu::flags::NC != 0 {
            prot |= NC;
        }
        if mmu_flags & mmu::flags::IO != 0 {
            prot |= IO;
        }
        prot
    }

    /// Convert prot flags into MMU flags.
    pub(super) const fn into_mmu_flags(prot_flags: u8) -> u32 {
        let mut mmu = 0;
        if prot_flags & READ != 0 {
            mmu |= mmu::flags::R;
        }
        if prot_flags & WRITE != 0 {
            mmu |= mmu::flags::W;
        }
        if prot_flags & EXEC != 0 {
            mmu |= mmu::flags::X;
        }
        if prot_flags & NC != 0 {
            mmu |= mmu::flags::NC;
        }
        if prot_flags & IO != 0 {
            mmu |= mmu::flags::IO;
        }
        mmu
    }
}

/// A page that is filled with zeroes.
static mut ZEROES: *const u8 = null();
/// Physical address of a page that is filled with zeroes.
static mut ZEROES_PADDR: PAddrr = 0;

/// Get the page that is filled with zeroes.
#[inline(always)]
pub fn zeroes() -> &'static [u8] {
    unsafe { &*core::ptr::slice_from_raw_parts(ZEROES, PAGE_SIZE as usize) }
}

/// Get the physical address of the page full of zeroes.
#[inline(always)]
pub fn zeroes_paddr() -> PAddrr {
    unsafe { ZEROES_PADDR }
}

/// Get a mappable page for the page full of zeroes.
#[inline(always)]
pub fn zeroes_page() -> MappablePage {
    unsafe { MappablePage::new(zeroes_paddr(), false, false, false) }
}

unsafe extern "C" {
    static __start_text: [u8; 0];
    static __stop_text: [u8; 0];
    static __start_rodata: [u8; 0];
    static __stop_rodata: [u8; 0];
    static __start_data: [u8; 0];
    static __stop_data: [u8; 0];

    /// Higher-half direct map virtual address.
    /// Provided by boot protocol.
    #[link_name = "vmm_hhdm_vaddr"]
    pub static mut HHDM_VADDR: usize;
    /// Higher-half direct map address offset (paddr -> vaddr).
    /// Provided by boot protocol.
    #[link_name = "vmm_hhdm_offset"]
    pub static mut HHDM_OFFSET: usize;
    /// Higher-half direct map size.
    /// Provided by boot protocol.
    #[link_name = "vmm_hhdm_size"]
    pub static mut HHDM_SIZE: usize;
    /// Kernel base virtual address.
    /// Provided by boot protocol.
    #[link_name = "vmm_kernel_vaddr"]
    pub static mut KERNEL_VADDR: usize;
    /// Kernel base physical address.
    /// Provided by boot protocol.
    #[link_name = "vmm_kernel_paddr"]
    pub static mut KERNEL_PADDR: usize;
}

/// The kernel memory map.
static mut KERNEL_MM: MaybeUninit<KernelVmSpace> = MaybeUninit::uninit();

/// Get the kernel memory map.
pub fn kernel_mm() -> &'static KernelVmSpace {
    unsafe { (&mut *&raw mut KERNEL_MM).assume_init_ref() }
}

/// Initialize the virtual-memory management subsystem.
pub unsafe fn init() {
    unsafe {
        mmu::early_init();

        let kernel_mm = KernelVmSpace::new();
        let k_v2p = KERNEL_PADDR.wrapping_sub(KERNEL_VADDR);

        // Kernel RX.
        let vpn = &raw const __start_text as usize;
        let size = &raw const __stop_text as usize - &raw const __start_text as usize;
        kernel_mm
            .0
            .map(
                size,
                vpn,
                0..0,
                map::FIXED | map::POPULATE | map::SHARED,
                prot::READ | prot::EXEC,
                Some(Mapping {
                    offset: 0,
                    object: Arc::new(RawMemory::new(vpn.wrapping_add(k_v2p), size)),
                }),
            )
            .expect("Failed to map kernel RX");

        // Kernel RO.
        let vpn = &raw const __start_rodata as usize;
        let size = &raw const __stop_rodata as usize - &raw const __start_rodata as usize;
        kernel_mm
            .0
            .map(
                size,
                vpn,
                0..0,
                map::FIXED | map::POPULATE | map::SHARED,
                prot::READ,
                Some(Mapping {
                    offset: 0,
                    object: Arc::new(RawMemory::new(vpn.wrapping_add(k_v2p), size)),
                }),
            )
            .expect("Failed to map kernel RO");

        // Kernel RW.
        let vpn = &raw const __start_data as usize;
        let size = &raw const __stop_data as usize - &raw const __start_data as usize;
        kernel_mm
            .0
            .map(
                size,
                vpn,
                0..0,
                map::FIXED | map::POPULATE | map::SHARED,
                prot::READ | prot::WRITE,
                Some(Mapping {
                    offset: 0,
                    object: Arc::new(RawMemory::new(vpn.wrapping_add(k_v2p), size)),
                }),
            )
            .expect("Failed to map kernel RW");

        // Higher-half direct map.
        // TODO: Support a sparse HHDM?
        kernel_mm
            .0
            .map(
                HHDM_SIZE,
                HHDM_VADDR,
                0..0,
                map::FIXED | map::POPULATE | map::SHARED,
                prot::READ | prot::WRITE,
                Some(Mapping {
                    offset: 0,
                    object: Arc::new(RawMemory::new(HHDM_VADDR - HHDM_OFFSET, HHDM_SIZE)),
                }),
            )
            .expect("Failed to create HHDM");

        // Page of zeroes.
        ZEROES_PADDR = pmm::page_alloc(0, pmm::PageUsage::KernelAnon)
            .expect("Failed to allocate page of zeroes");
        (*core::ptr::slice_from_raw_parts_mut(
            (ZEROES_PADDR + HHDM_OFFSET) as *mut u8,
            PAGE_SIZE as usize,
        ))
        .fill(0);
        ZEROES = kernel_mm
            .map(
                PAGE_SIZE as usize,
                0,
                map::SHARED,
                prot::READ,
                Some(Mapping {
                    offset: 0,
                    object: Arc::new(RawMemory::new(ZEROES_PADDR, PAGE_SIZE as usize)),
                }),
            )
            .expect("Failed to map page of zeroes") as *const u8;

        logkf!(LogLevel::Info, "Switching to own page tables");
        mmu::init(kernel_mm.0.pmap.root());
        KERNEL_MM = MaybeUninit::new(kernel_mm);
    }
}

vmm_ktest! { MAP_BASIC,
    unsafe {
        let size  = 0x8000;
        let vaddr = kernel_mm().map(size, 0, map::SHARED, prot::READ | prot::WRITE, None)?;

        // Some random accesses must succeed.
        let ptr = &mut *core::ptr::slice_from_raw_parts_mut(vaddr as *mut u8, size);
        for i in (0..size).step_by(PAGE_SIZE as usize) {
            let fault = fallible_store_u8(&raw mut ptr[i], 21);
            ktest_assert!(fault.is_ok());
        }

        kernel_mm().unmap(vaddr..vaddr+size)?;

        // Assert now that trying to access again traps.
        for i in (0..size).step_by(PAGE_SIZE as usize) {
            let fault = fallible_store_u8(&raw mut ptr[i], 21);
            ktest_assert!(fault.is_err());
        }
    }
}
