// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ops::{Deref, DerefMut};

use crate::{
    bindings::error::EResult,
    config::{self, PAGE_SIZE},
    mem::{self, vmm::kernel_mm},
};

use super::{PageUsage, phys_ptr::PhysPtr};

/// A box for physical RAM allocations.
pub struct PhysBox<T: Sized> {
    ptr: PhysPtr,
    vaddr: *mut T,
}
unsafe impl<T: Sized> Send for PhysBox<T> {}
unsafe impl<T: Sized + Sync> Sync for PhysBox<T> {}

impl<T: Sized> PhysBox<T> {
    /// Try to allocate some page-aligned physical memory and map it.
    pub unsafe fn try_new(io: bool, nc: bool) -> EResult<Self> {
        unsafe {
            let order = mem::pmm::size_to_order(size_of::<T>());
            let aligned_pages = mem::pmm::order_to_pages(order);
            let ptr = PhysPtr::new(order, PageUsage::KernelAnon)?;

            let flags = mem::vmm::flags::RW
                + mem::vmm::flags::A
                + mem::vmm::flags::D
                + io as u32 * mem::vmm::flags::IO
                + nc as u32 * mem::vmm::flags::NC;

            let vpn = kernel_mm().map_fixed(ptr.ppn(), None, aligned_pages, flags)?;

            let vaddr = (vpn * PAGE_SIZE as usize) as *mut T;
            core::ptr::write_bytes(vaddr as *mut u8, 0, aligned_pages);

            Ok(Self { ptr, vaddr })
        }
    }

    /// Get the underlying physical address.
    pub fn paddr(&self) -> usize {
        self.ptr.ppn() * config::PAGE_SIZE as usize
    }
}

impl<T: Sized> Deref for PhysBox<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.vaddr }
    }
}

impl<T: Sized> DerefMut for PhysBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.vaddr }
    }
}

impl<T: Sized> Drop for PhysBox<T> {
    fn drop(&mut self) {
        unsafe {
            let order = mem::pmm::size_to_order(size_of::<T>());
            let aligned_pages = mem::pmm::order_to_pages(order);
            let vpn = self.vaddr as usize / config::PAGE_SIZE as usize;

            kernel_mm().unmap(vpn..vpn + aligned_pages);
        }
    }
}
