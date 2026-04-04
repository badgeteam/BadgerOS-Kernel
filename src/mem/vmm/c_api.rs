// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{
    bindings::raw::errno_t,
    mem::{pmm::PPN, vmm::VPN},
};

#[unsafe(no_mangle)]
unsafe extern "C" fn vmm_map_k(
    virt_base_out: *mut VPN,
    virt_len: VPN,
    phys_base: PPN,
    flags: u32,
) -> errno_t {
    todo!()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vmm_map_k_at(
    virt_base: VPN,
    virt_len: VPN,
    phys_base: PPN,
    flags: u32,
) -> errno_t {
    todo!()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vmm_unmap_k(virt_base: VPN, virt_len: VPN) {
    todo!()
}
