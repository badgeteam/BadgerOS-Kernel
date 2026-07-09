// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{
    bindings::error::EResult,
    mem::{pmm::PAddrr, vmm::kernel_mm},
};

/// Scatter-gather list entry.
#[derive(Debug, Clone, Copy)]
pub struct ScatterGatherEntry {
    pub paddr: PAddrr,
    pub vaddr: usize,
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

/// Simpler reference wrapper struct that implements [`DmaBuffer`] by doing virt2phys lookups.
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
