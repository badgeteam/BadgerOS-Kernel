use core::{
    alloc::GlobalAlloc,
    ptr::{NonNull, null_mut},
};

use alloc::alloc::Allocator;

pub mod hhdm;
pub mod slabs;

struct Heap;

#[global_allocator]
static HEAP: Heap = Heap;

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let layout = layout.pad_to_align();

        let res = if layout.size() <= slabs::LARGEST {
            hhdm::HhdmAlloc.allocate(layout)
        } else {
            slabs::GLOBAL_SLABS.allocate(layout)
        };

        match res {
            Ok(ptr) => ptr.cast().as_ptr(),
            Err(_) => null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        unsafe {
            let ptr = NonNull::new_unchecked(ptr);

            if layout.size() <= slabs::LARGEST {
                hhdm::HhdmAlloc.deallocate(ptr, layout);
            } else {
                slabs::GLOBAL_SLABS.deallocate(ptr, layout);
            };
        }
    }
}
