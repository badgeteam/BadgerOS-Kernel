// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::usize;

use crate::{
    bindings::error::{EResult, Errno},
    config::PAGE_SIZE,
    mem::{
        pmm::PAddrr,
        vmm::{self, kernel_mm},
    },
};

/// Scatter-gather list entry.
#[derive(Debug, Clone, Copy)]
pub struct ScatterGatherEntry {
    pub paddr: PAddrr,
    pub vaddr: usize,
    // TODO: Should this be u64?
    pub size: usize,
}

/// DMA buffer; an object that can be collected into a scatter-gather list.
pub unsafe trait DmaTarget {
    /// DMA will write into host memory (modifying `self`).
    fn is_scatter(&self) -> bool;

    /// How many bytes there are to this slice.
    fn dma_size(&self) -> usize;

    /// Collect into scatter-gather list entries.
    fn collect(
        &self,
        max_entry_size: usize,
        sink: &mut dyn FnMut(ScatterGatherEntry) -> EResult<()>,
    ) -> EResult<()>;
}

/// Copy data into a DMA target using the CPU.
/// Fails if the `data` buffer is too small.
pub fn cpu_scatter(target: &dyn DmaTarget, data: &[u8]) -> EResult<()> {
    if !target.is_scatter() || target.dma_size() > data.len() {
        return Err(Errno::EINVAL);
    }

    let mut index = 0;
    let _ = target.collect(usize::MAX, &mut |ent| {
        let slice =
            unsafe { &mut *core::ptr::slice_from_raw_parts_mut(ent.vaddr as *mut u8, ent.size) };
        slice.copy_from_slice(&data[index..index + ent.size]);
        index += ent.size;
        Ok(())
    });

    Ok(())
}

/// Copy data out of a DMA target using the CPU.
/// Fails if the `data` buffer is too small.
pub fn cpu_gather(target: &dyn DmaTarget, data: &mut [u8]) -> EResult<()> {
    if target.is_scatter() || target.dma_size() > data.len() {
        return Err(Errno::EINVAL);
    }

    let mut index = 0;
    let _ = target.collect(usize::MAX, &mut |ent| {
        let slice = unsafe { &*core::ptr::slice_from_raw_parts(ent.vaddr as *mut u8, ent.size) };
        data[index..index + ent.size].copy_from_slice(slice);
        index += ent.size;
        Ok(())
    });

    Ok(())
}

/// Lets you gather zeroes from the zeroes page as a DMA target.
pub struct DmaFillZero(usize);

impl DmaFillZero {
    /// Create an arbitrarily long span of zeroes.
    pub const fn new(size: usize) -> Self {
        Self(size)
    }
}

unsafe impl DmaTarget for DmaFillZero {
    fn is_scatter(&self) -> bool {
        false
    }

    fn dma_size(&self) -> usize {
        self.0
    }

    fn collect(
        &self,
        max_entry_size: usize,
        sink: &mut dyn FnMut(ScatterGatherEntry) -> EResult<()>,
    ) -> EResult<()> {
        let max_entry_size = max_entry_size.min(PAGE_SIZE as usize);
        let vaddr = vmm::zeroes().as_ptr() as usize;
        let paddr = vmm::zeroes_paddr();
        let mut size = self.0;

        while size > 0 {
            let max = size.min(max_entry_size);
            sink(ScatterGatherEntry {
                paddr,
                vaddr,
                size: max,
            })?;
            size -= max;
        }

        Ok(())
    }
}

/// Implements [`DmaBuffer`] by associating an object with a given physical address.
pub struct DmaFromBuffer<'a, T: ?Sized + 'a, const IS_SCATTER: bool> {
    vaddr: &'a T,
    paddr: PAddrr,
}

unsafe impl<T: ?Sized, const IS_SCATTER: bool> DmaTarget for DmaFromBuffer<'_, T, IS_SCATTER> {
    fn is_scatter(&self) -> bool {
        IS_SCATTER
    }

    fn dma_size(&self) -> usize {
        size_of_val(self.vaddr)
    }

    fn collect(
        &self,
        max_entry_size: usize,
        sink: &mut dyn FnMut(ScatterGatherEntry) -> EResult<()>,
    ) -> EResult<()> {
        let mut vaddr = self.vaddr as *const _ as *const () as usize;
        let mut paddr = self.paddr;
        let mut size = size_of_val(self.vaddr);

        while size > 0 {
            let max = size.min(max_entry_size);
            sink(ScatterGatherEntry {
                paddr,
                vaddr,
                size: max,
            })?;
            vaddr += max;
            paddr += max;
            size -= max;
        }

        Ok(())
    }
}

/// Simple reference wrapper struct that implements [`DmaBuffer`] by doing virt2phys lookups.
#[repr(transparent)]
pub struct DmaFromRef<T: ?Sized, const IS_SCATTER: bool>(T);

impl<T: ?Sized> DmaFromRef<T, false> {
    pub const fn from_ref<'a>(that: &'a T) -> &'a Self {
        // SAFETY: DmaFromRef is a transparent wrapper around T.
        unsafe { core::mem::transmute(that) }
    }
}

impl<T: ?Sized> DmaFromRef<T, true> {
    pub const fn from_mut<'a>(that: &'a mut T) -> &'a mut Self {
        // SAFETY: DmaFromRef is a transparent wrapper around T.
        unsafe { core::mem::transmute(that) }
    }
}

unsafe impl<T: ?Sized, const IS_SCATTER: bool> DmaTarget for DmaFromRef<T, IS_SCATTER> {
    fn is_scatter(&self) -> bool {
        IS_SCATTER
    }

    fn dma_size(&self) -> usize {
        size_of_val(self)
    }

    fn collect(
        &self,
        max_entry_size: usize,
        sink: &mut dyn FnMut(ScatterGatherEntry) -> EResult<()>,
    ) -> EResult<()> {
        let mut vaddr = self as *const _ as *const () as usize;
        let mut size = size_of_val(self);

        let mut cur: Option<ScatterGatherEntry> = None;
        while size > 0 {
            let v2p = kernel_mm().virt2phys(vaddr);
            let mut ent_size = (v2p.size - (v2p.paddr - v2p.page_paddr)).min(size);

            if let Some(ent) = &mut cur {
                if ent_size != 0 && v2p.paddr == ent.paddr + ent.size {
                    ent_size = ent_size.min(max_entry_size - ent.size);
                    ent.size += ent_size;
                } else {
                    sink(*ent)?;
                    cur = Some(ScatterGatherEntry {
                        paddr: v2p.paddr,
                        vaddr,
                        size: ent_size,
                    });
                }
            } else {
                cur = Some(ScatterGatherEntry {
                    paddr: v2p.paddr,
                    vaddr,
                    size: ent_size,
                });
            }

            vaddr += ent_size;
            size -= ent_size;
        }

        if let Some(cur) = cur {
            sink(cur)?;
        }

        Ok(())
    }
}
