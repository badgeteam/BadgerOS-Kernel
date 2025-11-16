// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use super::*;

#[unsafe(no_mangle)]
unsafe extern "C" fn vmm_init() {
    unsafe {
        init();
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vmm_ctxswitch_from_isr() {
    unsafe {
        let proc = bindings::raw::proc_current();
        if proc.is_null() {
            cpu::mmu::set_page_table(KERNEL_MM.page_table, 0);
        } else {
            cpu::mmu::set_page_table((*proc).memmap.mem_ctx.pt_root_ppn, 0);
        }
        cpu::mmu::vmem_fence(None, None);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_create_user_ctx(ctx: *mut Memmap) -> errno_t {
    match create_user_ctx() {
        Ok(x) => {
            unsafe {
                *ctx = x;
            }
            0
        }
        Err(x) => -(x as errno_t),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_destroy_user_ctx(ctx: Memmap) {
    unsafe { destroy_user_ctx(ctx) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_map_k(
    virt_base_out: *mut VPN,
    virt_len: VPN,
    phys_base: PPN,
    flags: u32,
) -> errno_t {
    unsafe {
        match map_k(virt_len, phys_base, flags) {
            Ok(vpn) => {
                if !virt_base_out.is_null() {
                    *virt_base_out = vpn;
                }
                0
            }
            Err(e) => -(e as errno_t),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_map_k_at(
    virt_base: VPN,
    virt_len: VPN,
    phys_base: PPN,
    flags: u32,
) -> errno_t {
    Errno::extract(unsafe { map_k_at(virt_base, virt_len, phys_base, flags) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_unmap_k(virt_base: VPN, virt_len: VPN) -> errno_t {
    Errno::extract(unsafe { unmap_k(virt_base, virt_len) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_map_u_at(
    vmm_ctx: *mut Memmap,
    virt_base: VPN,
    virt_len: VPN,
    phys_base: PPN,
    flags: u32,
) -> errno_t {
    Errno::extract(unsafe { map_u_at(&*vmm_ctx, virt_base, virt_len, phys_base, flags) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_unmap_u(
    vmm_ctx: *mut Memmap,
    virt_base: VPN,
    virt_len: VPN,
) -> errno_t {
    Errno::extract(unsafe { unmap_u(&*vmm_ctx, virt_base, virt_len) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_virt2phys(mut vmm_ctx: *mut Memmap, vaddr: usize) -> virt2phys_t {
    if vmm_ctx.is_null() {
        vmm_ctx = &raw mut KERNEL_MM;
    }
    let tmp = unsafe { virt2phys(&*vmm_ctx, vaddr) };
    virt2phys_t {
        page_vaddr: tmp.page_vaddr,
        page_paddr: tmp.page_paddr,
        size: tmp.size,
        paddr: tmp.paddr,
        flags: tmp.flags,
        valid: tmp.valid,
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vmm_ctxswitch(ctx: *mut Memmap) {
    unsafe {
        cpu::mmu::set_page_table((*ctx).page_table, 0);
        cpu::mmu::vmem_fence(None, None);
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vmm_ctxswitch_k() {
    unsafe {
        cpu::mmu::set_page_table(KERNEL_MM.page_table, 0);
        cpu::mmu::vmem_fence(None, None);
    }
}
