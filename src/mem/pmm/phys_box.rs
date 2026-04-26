// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ops::{Deref, DerefMut};

use alloc::sync::Arc;

use crate::{
    bindings::error::EResult,
    config::PAGE_SIZE,
    cpu,
    mem::{
        pmm,
        vmm::{self, kernel_mm, map::Mapping, memobject::RawMemory},
    },
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
        let order = pmm::size_to_order(size_of::<T>());
        let aligned_pages = pmm::order_to_pages(order);
        let ptr = PhysPtr::new(order, PageUsage::KernelAnon)?;

        let prot = vmm::prot::READ
            | vmm::prot::WRITE + io as u8 * vmm::prot::IO + nc as u8 * vmm::prot::NC;

        let vaddr;
        unsafe {
            let object = Arc::try_new(RawMemory::new(
                ptr.paddr(),
                aligned_pages * PAGE_SIZE as usize,
            ))?;
            vaddr = kernel_mm().map(
                aligned_pages * PAGE_SIZE as usize,
                0,
                0,
                prot,
                Some(Mapping { offset: 0, object }),
            )? as *mut T;
            core::ptr::write_bytes(vaddr as *mut u8, 0, aligned_pages);
        }

        Ok(Self { ptr, vaddr })
    }

    /// Get the underlying physical address.
    pub fn paddr(&self) -> usize {
        self.ptr.paddr()
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
            let order = pmm::size_to_order(size_of::<T>());
            let aligned_size = pmm::order_to_size(order);
            let vaddr = self.vaddr as usize;

            kernel_mm()
                .unmap(vaddr..vaddr + aligned_size)
                .expect("PhysBox unmap failed");
        }
    }
}

vmm_ktest! { PHYS_BOX,
    const SIZE: usize = 0x2000;
    let mut mem = unsafe { PhysBox::<[u8; SIZE]>::try_new(false, false)? };

    let start_vma = &mem[0] as *const _ as usize;
    let start_pma = mem.paddr();
    for i in (0..SIZE).step_by(PAGE_SIZE as usize) {
        // Assert physically contiguous.
        let v2p = kernel_mm().virt2phys(start_vma + i);
        ktest_assert!(v2p.valid);
        ktest_assert!(v2p.flags & cpu::mmu::flags::W != 0);
        ktest_expect!(v2p.page_vaddr, start_vma + i);
        ktest_expect!(v2p.page_paddr, start_pma + i);

        // Test writability.
        mem[i] = 9;
    }
}
