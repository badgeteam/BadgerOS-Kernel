// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::sync::Arc;

use crate::{
    bindings::{error::Errno, raw::errno_t},
    mem::pmm::PAddrr,
};

use super::{
    kernel_mm,
    map::{self, Mapping},
    memobject::RawMemory,
};

#[unsafe(no_mangle)]
unsafe extern "C" fn vmm_map_k(
    virt_base_out: *mut usize,
    virt_len: usize,
    phys_base: PAddrr,
    flags: u32,
) -> errno_t {
    Errno::extract(
        try {
            unsafe {
                let object =
                    Arc::try_new(RawMemory::new(phys_base, virt_len)).map_err(Into::into)?;
                let vaddr = kernel_mm().map(
                    virt_len,
                    0,
                    0,
                    flags as _,
                    Some(Mapping { object, offset: 0 }),
                )?;
                *virt_base_out = vaddr;
            }
        },
    )
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vmm_map_k_at(
    virt_base: usize,
    virt_len: usize,
    phys_base: PAddrr,
    flags: u32,
) -> errno_t {
    Errno::extract(
        try {
            unsafe {
                let object =
                    Arc::try_new(RawMemory::new(phys_base, virt_len)).map_err(Into::into)?;
                kernel_mm().map(
                    virt_len,
                    virt_base,
                    map::FIXED,
                    flags as _,
                    Some(Mapping { object, offset: 0 }),
                )?;
            }
        },
    )
}

#[unsafe(no_mangle)]
unsafe extern "C" fn vmm_unmap_k(virt_base: usize, virt_len: usize) {
    unsafe {
        kernel_mm()
            .unmap(virt_base..virt_base + virt_len)
            .expect("Failed to unmap kernel anonymous mapping");
    }
}
