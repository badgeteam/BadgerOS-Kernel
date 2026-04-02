// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::bindings::{error::Errno, raw::errno_ppn_t};

use super::{
    PPN, Page, PageUsage, init, mark_free, page_alloc, page_free, page_struct, page_struct_base,
};

/// Allocate `1 << order` pages of physical memory.
#[unsafe(no_mangle)]
unsafe extern "C" fn pmm_page_alloc(order: u8, usage: PageUsage) -> errno_ppn_t {
    Errno::extract_usize(unsafe { page_alloc(order, usage) })
}

/// Free pages of physical memory.
#[unsafe(no_mangle)]
unsafe extern "C" fn pmm_page_free(block: PPN, order: u8) {
    unsafe { page_free(block, order) };
}

/// Get the `pmm_page_t` struct for some physical page number.
#[unsafe(no_mangle)]
unsafe extern "C" fn pmm_page_struct(page: PPN) -> *mut Page {
    page_struct(page)
}

/// Get the `pmm_page_t` struct for the start of the block that some physical page number lies in.
#[unsafe(no_mangle)]
unsafe extern "C" fn pmm_page_struct_base(page: PPN) -> *mut Page {
    page_struct_base(page).0
}

/// Mark a range of blocks as free.
#[unsafe(no_mangle)]
unsafe extern "C" fn pmm_mark_free(pages_start: PPN, pages_end: PPN) {
    unsafe { mark_free(pages_start..pages_end) };
}

/// Initialize the physical memory allocator.
/// It is assumed that the boot protocol implementation hereafter marks the kernel executable with [`PageUsage::KernelSegment`].
#[unsafe(no_mangle)]
unsafe extern "C" fn pmm_init(total_start: PPN, total_end: PPN, early_start: PPN, early_end: PPN) {
    unsafe { init(total_start..total_end, early_start..early_end) };
}
