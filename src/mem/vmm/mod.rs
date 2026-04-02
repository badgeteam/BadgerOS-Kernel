// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::AtomicUsize;

use map::KernelVmSpace;

use crate::mem::pmm::PPN;

pub mod map;
pub mod memobject;
pub mod pagecache;
pub mod physmap;

/// Mapping protection flags.
pub mod prot {
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
}

/// Unsigned integer that can store a virtual page number.
pub type AtomicVPN = AtomicUsize;
/// Unsigned integer that can store a virtual page number.
pub type VPN = usize;

/// Page number of a page that is filled with zeroes.
pub static mut PAGE_OF_ZEROES: PPN = 0;
/// Virtual address of the page of zeroes.
pub static mut ZEROES: *const [u8] = &[];

/// Get the page that is filled with zeroes.
pub fn zeroes() -> &'static [u8] {
    unsafe { &*ZEROES }
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

/// Get the kernel memory map.
pub fn kernel_mm() -> &'static KernelVmSpace {
    todo!()
}

/// Initialize the virtual-memory management subsystem.
pub unsafe fn init() {}
