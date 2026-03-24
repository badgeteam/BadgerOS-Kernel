// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::AtomicUsize;

pub mod map;
pub mod memmap;
pub mod memobject;
pub mod pagecache;
pub mod pmap;

/// Mapping protection flags.
pub mod prot {
    /// Mapping is readable.
    pub const READ: u32 = 1 << 0;
    /// Mapping is writable.
    pub const WRITE: u32 = 1 << 1;
    /// Mapping is executable.
    pub const EXEC: u32 = 1 << 2;
}

/// Unsigned integer that can store a virtual page number.
pub type AtomicVPN = AtomicUsize;
/// Unsigned integer that can store a virtual page number.
pub type VPN = usize;

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
