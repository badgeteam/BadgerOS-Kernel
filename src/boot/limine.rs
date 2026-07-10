// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::Ordering;

use limine::{BaseRevision, RequestsEndMarker, RequestsStartMarker, memmap, request::*};

#[cfg(target_arch = "riscv64")]
use crate::cpu;
use crate::{
    bindings::log::{LogLevel, write_unlocked},
    config::PAGE_SIZE,
    mem::{
        pmm::{self, PAddrr},
        vmm,
    },
};

// Aarch64 and loongarch64 have critical problems before base revision 6, but RISC-V and x86_64 do not.
// Therefor, we accept a slightly older protocol for the latter two.
#[cfg(not(any(target_arch = "aarch64", target_arch = "loongarch64")))]
const BADGEROS_MIN_BASE_REV: u64 = 5;
#[cfg(any(target_arch = "aarch64", target_arch = "loongarch64"))]
const BADGEROS_MIN_BASE_REV: u64 = 6;

#[unsafe(link_section = ".requests_start")]
static START_MARKER: RequestsStartMarker = RequestsStartMarker::new();
#[unsafe(link_section = ".requests_end")]
static END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[unsafe(link_section = ".requests")]
static HHDM: HhdmRequest = HhdmRequest::new();
#[unsafe(link_section = ".requests")]
static MEMMAP: MemmapRequest = MemmapRequest::new();
#[unsafe(link_section = ".requests")]
static KADDR: ExecutableAddressRequest = ExecutableAddressRequest::new();
#[unsafe(link_section = ".requests")]
static CMDLINE: ExecutableCmdlineRequest = ExecutableCmdlineRequest::new();
#[unsafe(link_section = ".requests")]
static KFILE: ExecutableFileRequest = ExecutableFileRequest::new();
#[cfg(feature = "dtb")]
#[unsafe(link_section = ".requests")]
static DTB: DtbRequest = DtbRequest::new();
#[cfg(feature = "acpi")]
#[unsafe(link_section = ".requests")]
static RSDP: RsdpRequest = RsdpRequest::new();

/// Pre-heap boot protocol code.
pub unsafe fn early_init() {
    write_unlocked("\x1b[0m\x1b[2J");

    if !BASE_REVISION.is_supported() {
        panic!(
            "Minimum base revision ({}) not supported; actual: {:?}",
            BADGEROS_MIN_BASE_REV,
            BASE_REVISION.actual_revision()
        );
    }

    // Assert that the mandatory requests are satisfied.
    let hhdm = HHDM.response().expect("HHDM response is missing");
    let memmap = MEMMAP.response().expect("Memory map response is missing");
    let kaddr = KADDR
        .response()
        .expect("Executable address response is missing");
    assert!(
        kaddr.physical_base % PAGE_SIZE as u64 == 0,
        "Kernel is not aligned to page address"
    );

    unsafe { vmm::HHDM_OFFSET = hhdm.offset as usize };

    // Analyze the memory map.
    let mut hhdm_min_paddr = PAddrr::MAX;
    let mut hhdm_max_paddr = 0;
    let mut kseg = 0;
    let mut usable = 0;
    let mut reclaim = 0;
    let mut biggest_usable: Option<&memmap::Entry> = None;
    for &entry in memmap.entries() {
        use limine::memmap::*;
        let type_str = match entry.type_ {
            MEMMAP_USABLE => "Usable",
            MEMMAP_RESERVED => "Resvd",
            MEMMAP_ACPI_RECLAIMABLE => "ACPI reclaim",
            MEMMAP_ACPI_NVS => "ACPI NVS",
            MEMMAP_BAD_MEMORY => "Bad",
            MEMMAP_BOOTLOADER_RECLAIMABLE => "BL reclaim",
            MEMMAP_EXECUTABLE_AND_MODULES => "Kernel",
            MEMMAP_FRAMEBUFFER => "Framebuffer",
            MEMMAP_MAPPED_RESERVED => "Resvd (mapped)",
            _ => "???",
        };
        logkf_unlocked!(
            LogLevel::Info,
            "0x{:016x}-0x{:016x}  {}",
            entry.base,
            entry.base + entry.length - 1,
            type_str
        );
        match entry.type_ {
            MEMMAP_USABLE => usable += entry.length as usize,
            MEMMAP_ACPI_RECLAIMABLE | MEMMAP_BOOTLOADER_RECLAIMABLE => {
                reclaim += entry.length as usize
            }
            _ => (),
        }
        match entry.type_ {
            MEMMAP_USABLE
            | MEMMAP_ACPI_RECLAIMABLE
            | MEMMAP_ACPI_NVS
            | MEMMAP_BOOTLOADER_RECLAIMABLE
            | MEMMAP_EXECUTABLE_AND_MODULES
            | MEMMAP_MAPPED_RESERVED => {
                hhdm_min_paddr = hhdm_min_paddr.min(entry.base as PAddrr);
                hhdm_max_paddr = hhdm_max_paddr.max((entry.base + entry.length) as PAddrr);
            }
            _ => (),
        }
        if entry.type_ == MEMMAP_EXECUTABLE_AND_MODULES {
            kseg += entry.length as usize;
        }

        if entry.type_ == MEMMAP_USABLE && biggest_usable.map_or(true, |x| x.length < entry.length)
        {
            biggest_usable = Some(entry);
        }
    }
    let biggest_usable = biggest_usable.expect("No usable memory");

    logkf_unlocked!(
        LogLevel::Info,
        "Total:   {} MiB",
        (usable + reclaim + kseg) / 1024 / 1024
    );
    logkf_unlocked!(LogLevel::Info, "Usable:  {} MiB", usable / 1024 / 1024);
    logkf_unlocked!(LogLevel::Info, "Reclaim: {} MiB", reclaim / 1024 / 1024);

    // Set up PMM.
    unsafe {
        pmm::init(
            hhdm_min_paddr..hhdm_max_paddr,
            biggest_usable.base as PAddrr..(biggest_usable.base + biggest_usable.length) as PAddrr,
        );
    }
    for &entry in memmap.entries() {
        use limine::memmap::*;
        if core::ptr::addr_eq(entry, biggest_usable) {
            continue;
        }
        unsafe {
            let pages = entry.length as usize / PAGE_SIZE as usize;
            match entry.type_ {
                MEMMAP_USABLE => {
                    pmm::mark_free(entry.base as PAddrr..(entry.base + entry.length) as PAddrr);
                }
                MEMMAP_EXECUTABLE_AND_MODULES => {
                    (*pmm::page_struct(entry.base as PAddrr))
                        .set_usage(pmm::PageUsage::KernelSegment);
                    pmm::KERNEL_PAGES.fetch_add(pages, Ordering::Relaxed);
                    pmm::USED_PAGES.fetch_add(pages, Ordering::Relaxed);
                }
                _ => (),
            }
        }
    }

    unsafe {
        vmm::HHDM_SIZE = hhdm_max_paddr - hhdm_min_paddr;
        vmm::HHDM_VADDR = hhdm_min_paddr + vmm::HHDM_OFFSET;
        vmm::KERNEL_PADDR = kaddr.physical_base as usize;
        vmm::KERNEL_VADDR = kaddr.virtual_base as usize;
    }
}

/// Post-heap boot protocol code.
pub unsafe fn late_init() {}

/// Get RSDP physical address.
#[cfg(feature = "dtb")]
pub fn get_rsdp_paddr() -> PAddrr {
    let Some(rsdp) = RSDP.response() else {
        return 0;
    };
    rsdp.address as usize - HHDM.response().unwrap().offset as usize
}

/// Get FDT pointer, if any.
#[cfg(feature = "dtb")]
pub fn get_fdt_ptr() -> *const () {
    DTB.response().map_or(core::ptr::null(), |x| x.dtb_ptr)
}

/// Reclaim bootloader memory.
pub unsafe fn reclaim_mem() {}

/// Legacy, to be replaced with proper earlycon later on.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bootp_early_putc(c: u8) {
    #[cfg(target_arch = "riscv64")]
    {
        let _ = cpu::sbi::legacy::console_putchar(c);
    }
    #[cfg(target_arch = "x86_64")]
    core::arch::asm! {
        "out dx, al",
        in("dx") 0x3f8,
        in("al") c
    }
}
