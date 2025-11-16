// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    fmt::Debug,
    ops::Div,
    ptr::slice_from_raw_parts,
    sync::atomic::{Atomic, AtomicUsize, Ordering},
};

use mmu::PTE_PER_PAGE;

use crate::{
    bindings::{
        self,
        error::{EResult, Errno},
        log::LogLevel,
        mutex::{Mutex, SharedMutexGuard},
        raw::{errno_t, virt2phys_t},
        spinlock::Spinlock,
    },
    config::PAGE_SIZE,
    cpu::{
        self,
        mmu::{BITS_PER_LEVEL, PackedPTE},
    },
    mem::{
        pmm::{PPN, page_alloc},
        vmm::{mmu::PAGING_LEVELS, vmaalloc::VmaAlloc},
    },
};

use super::pmm;

mod c_api;
pub mod mmu;
mod vmaalloc;

pub mod flags {
    // Note: These flags are the same bit positions as in the RISC-V PTE format, do not change them!

    /// Map memory as executable.
    pub const R: u32 = 0b0000_0000_0010;
    /// Map memory as writeable (reads must also be allowed).
    pub const W: u32 = 0b0000_0000_0100;
    /// Map memory as executable.
    pub const X: u32 = 0b0000_0000_1000;
    /// Map memory as user-accessible.
    pub const U: u32 = 0b0000_0001_0000;
    /// Map memory as global (exists in all page ASIDs).
    pub const G: u32 = 0b0000_0010_0000;
    /// Page was accessed since this flag was last cleared.
    pub const A: u32 = 0b0000_0100_0000;
    /// Page was written since this flag was last cleared.
    pub const D: u32 = 0b0000_1000_0000;

    /// Mark page as copy-on-write (W must be disabled).
    pub const COW: u32 = 0b0001_0000_0000;

    /// Map memory as I/O (uncached, no write coalescing).
    pub const IO: u32 = 0b0100_0000_0000;
    /// Map memory as uncached write coalescing.
    pub const NC: u32 = 0b1000_0000_0000;

    /// Map memory as read-write.
    pub const RW: u32 = R | W;
    /// Map memory as read-execute.
    pub const RX: u32 = R | X;
    /// Map memory as read-write-execute.
    pub const RWX: u32 = R | W | X;
}

/// Unsigned integer that can store a virtual page number.
pub type AtomicVPN = AtomicUsize;

/// Unsigned integer that can store a virtual page number.
pub type VPN = usize;

/// Naturally aligned slice that is a page or more of zeroes.
static mut ZEROES: *const [u8] = unsafe { core::mem::zeroed() };

/// Kernel page table root PPN.
pub static mut KERNEL_MM: Memmap = Memmap {
    is_kernel: true,
    page_table: unsafe { Spinlock::new_static(0) },
    vma_alloc: unsafe { Mutex::new_static(VmaAlloc::new()) },
};

/// Mutex guarding modifications to the kernel page table.
pub static KERNEL_VMM_MTX: Mutex<()> = unsafe { Mutex::new_static(()) };

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

// Memory management context.
#[repr(C)]
pub struct Memmap {
    /// The kernel memmap isn't allowed to do certain things,
    /// so this makes that distinction.
    is_kernel: bool,
    page_table: Spinlock<PPN>,
    vma_alloc: Mutex<VmaAlloc>,
}

/// Calculates the maximum page table level wherein the next part of the mapping can be made.
fn calc_superpage(virt_base: VPN, virt_len: VPN, phys_base: PPN) -> u8 {
    let align = virt_base | phys_base;
    ((align.trailing_zeros().min(virt_len.ilog2()) / BITS_PER_LEVEL as u32) as u8)
        .min(unsafe { PAGING_LEVELS as u8 })
}

/// Common implementation of the various (un)map functions.
unsafe fn map_impl(
    ctx: &Memmap,
    virt_base: VPN,
    virt_len: VPN,
    phys_base: PPN,
    flags: u32,
) -> EResult<()> {
    // Map in original page table.
    let mut offset = 0 as VPN;
    let mut root_touched = false;
    while offset < virt_len {
        let level = calc_superpage(virt_base + offset, virt_len - offset, phys_base + offset);
        root_touched |= unsafe {
            mmu::map(
                ctx.page_table,
                virt_base + offset,
                mmu::PTE {
                    ppn: phys_base + offset,
                    flags,
                    level,
                    valid: flags & flags::RWX != 0,
                    leaf: true,
                },
            )
        }?;
        offset += (1 as VPN) << (BITS_PER_LEVEL * level as u32);
    }

    if root_touched && (flags & flags::G) != 0 {
        let root_pte_size = 1usize << ((unsafe { PAGING_LEVELS } - 1) * BITS_PER_LEVEL);

        // Broadcast global mappings to process page tables.
        let guard = unsafe {
            SharedMutexGuard::new_raw(
                &raw mut bindings::raw::proc_mtx,
                slice_from_raw_parts(bindings::raw::procs, bindings::raw::procs_len),
            )
        };
        for proc in guard.iter() {
            let proc_pt_root = unsafe { (**proc).memmap.mem_ctx.pt_root_ppn };
            for index in
                virt_base.div(root_pte_size)..(virt_base + virt_len).div_ceil(root_pte_size)
            {
                unsafe { mmu::xchg_pte(proc_pt_root, index, mmu::read_pte(ctx.page_table, index)) };
            }
        }
    }

    // Local TLB invalidation.
    let inval_range = virt_base..virt_base + virt_len;
    if inval_range.len() > cpu::mmu::INVAL_PAGE_THRESHOLD {
        cpu::mmu::vmem_fence(None, None);
    } else {
        for page in inval_range {
            cpu::mmu::vmem_fence(Some(page * PAGE_SIZE as usize), None);
        }
    }

    // TODO: Remote TLB invalidation dispatch.

    Ok(())
}

/// Map a range of memory for the kernel at a specific virtual address.
/// # Safety
/// - The caller is responsible for ensuring the safety of `virt_base`;
/// - The caller is responsible for managing the memory at `phys_base`.
pub unsafe fn map_k_at(virt_base: VPN, virt_len: VPN, phys_base: PPN, flags: u32) -> EResult<()> {
    if flags & flags::U != 0
        || !mmu::is_canon_kernel_range(
            virt_base * PAGE_SIZE as usize..(virt_base + virt_len) * PAGE_SIZE as usize,
        )
    {
        return Err(Errno::EINVAL);
    }
    let flags = flags | flags::G;
    let _guard = KERNEL_VMM_MTX.lock();
    unsafe {
        map_impl(
            &*&raw const KERNEL_MM,
            virt_base,
            virt_len,
            phys_base,
            flags,
        )
    }
}

/// Finds a certain length of free pages within the higher half.
/// Makes sure there is at least one unmapped page before and after the found region.
/// # Safety
/// - The caller is responsible for preventing a concurrent map/unmap in the same page table.
unsafe fn find_free_pages(pt_root_ppn: PPN, virt_len: VPN) -> EResult<VPN> {
    let mut vpn = (mmu::higher_half_vaddr() / PAGE_SIZE as usize) as VPN;
    let limit = vpn + (mmu::canon_half_size() / PAGE_SIZE as usize) as VPN;
    let mut start_vpn = 0 as VPN;
    let mut avl_len = 0 as VPN;

    while vpn < limit {
        let pte = unsafe { mmu::walk(pt_root_ppn, vpn) };
        let pte_len = 1 << (pte.level as VPN * BITS_PER_LEVEL as VPN);
        if pte.valid {
            avl_len = 0;
        } else {
            if avl_len == 0 {
                start_vpn = vpn;
            }
            avl_len += pte_len;
        }
        if avl_len >= virt_len + 2 {
            return Ok(start_vpn + 1);
        }
        vpn += pte_len;
    }

    Err(Errno::ENOMEM)
}

/// Map a range of memory for the kernel at any virtual address.
/// Returns the virtual page number where it was mapped.
/// # Safety
/// - The caller is responsible for managing the memory at `phys_base`.
pub unsafe fn map_k(virt_len: VPN, phys_base: PPN, flags: u32) -> EResult<VPN> {
    if flags & flags::U != 0 {
        return Err(Errno::EINVAL);
    }
    let flags = flags | flags::G;
    let _guard = KERNEL_VMM_MTX.lock();
    let virt_base = unsafe { find_free_pages(KERNEL_MM.page_table, virt_len) }?;
    unsafe {
        map_impl(
            &*&raw const KERNEL_MM,
            virt_base,
            virt_len,
            phys_base,
            flags,
        )
    }?;
    Ok(virt_base)
}

/// Unmap a range of kernel memory.
/// # Safety
/// - The caller is responsible for ensuring the unmapped memory is no longer in use.
pub unsafe fn unmap_k(virt_base: VPN, virt_len: VPN) -> EResult<()> {
    if !mmu::is_canon_kernel_range(
        virt_base * PAGE_SIZE as usize..(virt_base + virt_len) * PAGE_SIZE as usize,
    ) {
        return Err(Errno::EINVAL);
    }
    unsafe { map_impl(&*&raw const KERNEL_MM, virt_base, virt_len, 0, 0) }
}

/// Map a range of memory for a user page table at a specific virtual address.
/// # Safety
/// - The caller is responsible for managing the memory at `phys_base`;
/// - The caller is responsible for preventing a concurrent map/unmap in the same page table.
pub unsafe fn map_u_at(
    ctx: &Memmap,
    virt_base: VPN,
    virt_len: VPN,
    phys_base: PPN,
    flags: u32,
) -> EResult<()> {
    if flags & !(flags::RWX | flags::A | flags::D) != 0
        || !mmu::is_canon_user_range(
            virt_base * PAGE_SIZE as usize..(virt_base + virt_len) * PAGE_SIZE as usize,
        )
    {
        return Err(Errno::EINVAL);
    }
    let flags = flags | flags::U;
    unsafe { map_impl(ctx, virt_base, virt_len, phys_base, flags) }
}

/// Unmap a range of user memory.
/// # Safety
/// - The caller is responsible for preventing a concurrent map/unmap in the same page table.
pub unsafe fn unmap_u(ctx: &Memmap, virt_base: VPN, virt_len: VPN) -> EResult<()> {
    if !mmu::is_canon_user_range(
        virt_base * PAGE_SIZE as usize..(virt_base + virt_len) * PAGE_SIZE as usize,
    ) {
        return Err(Errno::EINVAL);
    }
    unsafe { map_impl(ctx, virt_base, virt_len, 0, 0) }
}

/// Create a user virtual memory context.
pub fn create_user_ctx() -> EResult<Memmap> {
    unsafe {
        let pt_root_ppn = page_alloc(0, pmm::PageUsage::PageTable)?;

        // Copy the current kernel mappings into this new user page table.
        let new_pt_hhdm = &*slice_from_raw_parts(
            (pt_root_ppn * PAGE_SIZE as usize + HHDM_OFFSET) as *const Atomic<PackedPTE>,
            PTE_PER_PAGE,
        );
        let kernel_pt_hhdm = &*slice_from_raw_parts(
            (KERNEL_MM.page_table * PAGE_SIZE as usize + HHDM_OFFSET) as *const Atomic<PackedPTE>,
            PTE_PER_PAGE,
        );
        for i in 0..PTE_PER_PAGE {
            new_pt_hhdm[i].store(kernel_pt_hhdm[i].load(Ordering::Acquire), Ordering::Release);
        }

        Ok(Memmap {
            page_table: pt_root_ppn,
        })
    }
}

/// Destroy a user virtual memory context.
/// # Safety
/// - The caller is responsible for ensuring the context is no longer in use.
pub unsafe fn destroy_user_ctx(_ctx: Memmap) {
    logkf!(LogLevel::Warning, "TODO: vmm::destroy_user_ctx");
}

/// Describes the result of a virtual to physical address translation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Virt2Phys {
    /// Virtual address of page start.
    pub page_vaddr: VPN,
    /// Physical address of page start.
    pub page_paddr: PPN,
    /// Size of the mapping in pages.
    pub size: usize,
    /// Physical address.
    pub paddr: usize,
    /// Flags of the mapping.
    pub flags: u32,
    /// Whether the mapping exists; if false, only `vpn` and `size` are valid.
    pub valid: bool,
}

impl Debug for Virt2Phys {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use flags::*;
        f.debug_struct("Virt2Phys")
            .field("page_vaddr", &format_args!("0x{:x}", self.page_vaddr))
            .field("page_paddr", &format_args!("0x{:x}", self.page_paddr))
            .field("size", &format_args!("0x{:x}", self.size))
            .field("paddr", &format_args!("0x{:x}", self.paddr))
            .field(
                "flags",
                &format_args!(
                    "0x{:x} /* {}{}{}{}{}{}{} {} {} */",
                    self.flags,
                    if self.flags & R != 0 { 'R' } else { '-' },
                    if self.flags & W != 0 { 'W' } else { '-' },
                    if self.flags & X != 0 { 'X' } else { '-' },
                    if self.flags & U != 0 { 'U' } else { '-' },
                    if self.flags & G != 0 { 'G' } else { '-' },
                    if self.flags & A != 0 { 'A' } else { '-' },
                    if self.flags & D != 0 { 'D' } else { '-' },
                    if self.flags & COW != 0 { "COW" } else { "---" },
                    if self.flags & IO != 0 {
                        "IO"
                    } else if self.flags & NC != 0 {
                        "NC"
                    } else {
                        "--"
                    }
                ),
            )
            .field("valid", &self.valid)
            .finish()
    }
}

/// Translate a virtual to a physical address.
pub unsafe fn virt2phys(ctx: &Memmap, vaddr: usize) -> Virt2Phys {
    if !mmu::is_canon_addr(vaddr) {
        logkf!(
            LogLevel::Warning,
            "Tried to look up non-canonical virtual address"
        );
        return Virt2Phys {
            page_vaddr: vaddr & !(PAGE_SIZE as usize - 1),
            page_paddr: 0,
            size: PAGE_SIZE as usize,
            paddr: 0,
            flags: 0,
            valid: false,
        };
    }

    let pte = unsafe { mmu::walk(ctx.page_table, vaddr / PAGE_SIZE as usize) };

    let size = (PAGE_SIZE as usize) << (BITS_PER_LEVEL * pte.level as u32);
    let page_vaddr = vaddr & !(size - 1);
    let page_paddr = pte.ppn * PAGE_SIZE as usize;
    let offset = vaddr - page_vaddr;
    Virt2Phys {
        page_vaddr,
        page_paddr,
        size,
        paddr: page_paddr + offset,
        flags: pte.flags,
        valid: pte.valid,
    }
}

/// Initialize the virtual memory subsystem.
pub unsafe fn init() {
    unsafe {
        // Detect MMU mode to be used.
        cpu::mmu::early_init();
        logkf!(LogLevel::Info, "MMU paging levels: {}", { PAGING_LEVELS });

        // Prepare new page tables containing kernel and HHDM.
        let res: EResult<()> = try {
            // Allocate and zero out page table.
            let pt_root_ppn = mmu::alloc_pgtable_page()?;
            KERNEL_MM = Memmap {
                page_table: pt_root_ppn,
            };

            // Kernel RX.
            let text_vaddr = &raw const __start_text as usize;
            let text_len = &raw const __stop_text as usize - &raw const __start_text as usize;
            debug_assert!(text_len % PAGE_SIZE as usize == 0);
            map_k_at(
                text_vaddr / PAGE_SIZE as usize,
                text_len / PAGE_SIZE as usize,
                (text_vaddr - KERNEL_VADDR + KERNEL_PADDR) / PAGE_SIZE as usize,
                flags::RX | flags::G | flags::A | flags::D,
            )?;

            // Kernel R.
            let rodata_vaddr = &raw const __start_rodata as usize;
            let rodata_len = &raw const __stop_rodata as usize - &raw const __start_rodata as usize;
            debug_assert!(rodata_len % PAGE_SIZE as usize == 0);
            map_k_at(
                rodata_vaddr / PAGE_SIZE as usize,
                rodata_len / PAGE_SIZE as usize,
                (rodata_vaddr - KERNEL_VADDR + KERNEL_PADDR) / PAGE_SIZE as usize,
                flags::R | flags::G | flags::A | flags::D,
            )?;

            // Kernel RW.
            let data_vaddr = &raw const __start_data as usize;
            let data_len = &raw const __stop_data as usize - &raw const __start_data as usize;
            debug_assert!(data_len % PAGE_SIZE as usize == 0);
            map_k_at(
                data_vaddr / PAGE_SIZE as usize,
                data_len / PAGE_SIZE as usize,
                (data_vaddr - KERNEL_VADDR + KERNEL_PADDR) / PAGE_SIZE as usize,
                flags::RW | flags::G | flags::A | flags::D,
            )?;

            // HHDM RW.
            debug_assert!(HHDM_SIZE % PAGE_SIZE as usize == 0);
            map_k_at(
                HHDM_VADDR / PAGE_SIZE as usize,
                HHDM_SIZE / PAGE_SIZE as usize,
                (HHDM_VADDR - HHDM_OFFSET) / PAGE_SIZE as usize,
                flags::RW | flags::G | flags::A | flags::D,
            )?;

            // Page of zeroes.
            let zeroes_order = 0;
            let zeroes_ppn = pmm::page_alloc(zeroes_order, pmm::PageUsage::KernelAnon)?;
            let zeroes_vpn = map_k(
                1 << zeroes_order,
                zeroes_ppn,
                flags::R | flags::G | flags::A | flags::D,
            )?;
            ZEROES = slice_from_raw_parts(
                (zeroes_vpn * PAGE_SIZE as usize) as *const u8,
                (PAGE_SIZE as usize) << zeroes_order,
            );
            (*(ZEROES as *mut [u8])).fill(0);
        };
        res.expect("Failed to create inital page table");

        // Finalize MMU initialization and switch to new page table.
        logkf!(LogLevel::Info, "Switching to new page table");
        cpu::mmu::init(KERNEL_MM.page_table);
        logkf!(LogLevel::Info, "Virtual memory management initialized");
    }
}

/// Returns at least one page of zeroed memory.
pub fn zeroes() -> &'static [u8] {
    unsafe {
        assert!(!ZEROES.is_null());
        &*ZEROES
    }
}
