use core::ptr::NonNull;

use alloc::alloc::{AllocError, Allocator};

use crate::{
    config::PAGE_SIZE,
    mem::{pmm, vmm},
};

pub struct HhdmAlloc;

unsafe impl Allocator for HhdmAlloc {
    fn allocate(&self, layout: core::alloc::Layout) -> Result<NonNull<[u8]>, AllocError> {
        let order = pmm::size_to_order(layout.pad_to_align().size());
        unsafe {
            pmm::page_alloc(order, pmm::PageUsage::KernelAnon).map(|paddr| {
                NonNull::new_unchecked(core::ptr::slice_from_raw_parts_mut(
                    (paddr + vmm::HHDM_OFFSET) as _,
                    (PAGE_SIZE as usize) << order,
                ))
            })
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: core::alloc::Layout) {
        let order = pmm::size_to_order(layout.pad_to_align().size());
        unsafe {
            pmm::page_free(ptr.addr().get(), order);
        }
    }
}
