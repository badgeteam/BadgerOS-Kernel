// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::bindings::{
    error::Errno,
    raw::{errno_t, virt2phys_t},
};

use super::*;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_destroy_user_ctx(ctx: Memmap) {
    drop(ctx);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_map_k(
    virt_base_out: *mut VPN,
    virt_len: VPN,
    phys_base: PPN,
    flags: u32,
) -> errno_t {
    unsafe {
        match kernel_mm().map_fixed(phys_base, None, virt_len, flags) {
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
    unsafe {
        match kernel_mm().map_fixed(phys_base, Some(virt_base), virt_len, flags) {
            Ok(_) => 0,
            Err(e) => -(e as errno_t),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_unmap_k(virt_base: VPN, virt_len: VPN) {
    unsafe {
        kernel_mm().unmap(virt_base..virt_base + virt_len);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_map_u(
    vmm_ctx: *mut Memmap,
    virt_base_out: *mut VPN,
    virt_len: VPN,
    phys_base: PPN,
    flags: u32,
) -> errno_t {
    unsafe {
        match (*vmm_ctx).map_fixed(phys_base, None, virt_len, flags) {
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
pub unsafe extern "C" fn vmm_map_u_at(
    vmm_ctx: *mut Memmap,
    virt_base: VPN,
    virt_len: VPN,
    phys_base: PPN,
    flags: u32,
) -> errno_t {
    Errno::extract(unsafe {
        (*vmm_ctx)
            .map_fixed(phys_base, Some(virt_base), virt_len, flags)
            .map(|_| ())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_unmap_u(vmm_ctx: *mut Memmap, virt_base: VPN, virt_len: VPN) {
    unsafe {
        (*vmm_ctx).unmap(virt_base..virt_base + virt_len);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmm_virt2phys(mut vmm_ctx: *mut Memmap, vaddr: usize) -> virt2phys_t {
    if vmm_ctx.is_null() {
        vmm_ctx = kernel_mm() as *const Memmap as *mut Memmap;
    }
    let tmp = unsafe { (*vmm_ctx).virt2phys(vaddr) };
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
        mmu::set_page_table((*ctx).pagetable.root_ppn(), 0);
        mmu::vmem_fence(None, None);
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vmm_ctxswitch_k() {
    unsafe {
        mmu::set_page_table(kernel_mm().pagetable.root_ppn(), 0);
        mmu::vmem_fence(None, None);
    }
}
