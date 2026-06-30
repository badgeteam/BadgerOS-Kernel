// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    any::type_name,
    marker::PhantomData,
    ops::{Deref, DerefMut, Range},
};

use alloc::{boxed::Box, sync::Arc};
use dtb::DtbNode;

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    config::PAGE_SIZE,
    dev2::{Device, bus::Bus, class::irqctl::IrqCtlDevice},
    mem::{
        pmm::PAddrr,
        vmm::{
            kernel_mm,
            map::{self, Mapping},
            memobject::RawMemory,
            prot,
        },
    },
};

use super::BusBase;

#[derive(Clone)]
pub enum SocIrqParent {
    /// Parent interrupt controller is the CPU interrupt controller.
    Cpu(u32),
    /// Parent interrupt controller is a device.
    Device(Arc<dyn IrqCtlDevice>),
}

/// Interrupt route for [`MmioBus`].
#[derive(Clone)]
pub struct SocIrqExt {
    /// Parent inerrupt controller.
    pub irqctl: SocIrqParent,
    /// Interrupt vector of the interrupt controller.
    pub vector: u128,
}

/// System-on-chip memory-mapped I/O bus.
pub struct SocBus {
    /// Base bus struct.
    base: BusBase,
    /// Associated DTB node, if any.
    dtb_node: Option<&'static DtbNode>,
    /// Physical addresses in this MMIO bus.
    paddr: Box<[Range<PAddrr>]>,
    /// Extended interrupts map.
    irq_ext: Box<[SocIrqExt]>,
}

impl SocBus {
    pub fn new(
        dtb_node: Option<&'static DtbNode>,
        paddr: Box<[Range<PAddrr>]>,
        irq_ext: Box<[SocIrqExt]>,
    ) -> Self {
        Self {
            base: BusBase::new(),
            dtb_node,
            paddr,
            irq_ext,
        }
    }

    pub const fn len(&self) -> usize {
        self.paddr.len()
    }

    pub fn map(&self, slot: usize) -> EResult<MmioMapping> {
        MmioMapping::new(self.paddr[slot].start, self.paddr[slot].len())
    }

    pub fn paddr(&self) -> &[Range<PAddrr>] {
        &self.paddr
    }

    pub fn irq_ext(&self) -> &[SocIrqExt] {
        &self.irq_ext
    }
}

impl Bus for SocBus {
    fn base(&self) -> &BusBase {
        &self.base
    }

    fn parent_device(&self) -> Option<Arc<dyn Device>> {
        None
    }

    fn dtb_node(&self) -> Option<&'static DtbNode> {
        self.dtb_node
    }

    unsafe fn install_irq(&self, dev_irq: u128, handler: *const dyn Device) -> EResult<()> {
        if dev_irq >= self.irq_ext.len() as u128 {
            return Err(Errno::EINVAL);
        }

        let irq = &self.irq_ext[dev_irq as usize];
        let SocIrqParent::Device(irqctl) = &irq.irqctl else {
            // For various reasons, installing an IRQ directly is not technically feasible.
            // In addition, on most platforms, all devices connect through a platform-level interrupt controller first anyway.
            logkf!(
                LogLevel::Error,
                "Cannot install IRQ directly on CPU interrupt controller"
            );
            return Err(Errno::EINVAL);
        };
        // SAFETY: forwarded from the caller of `Bus::install_irq`, same contract.
        unsafe {
            irqctl
                .irqctl_base()
                .install_irq(irq.vector, dev_irq, handler)?
        };
        irqctl.set_irq_in_enabled(irq.vector, true)
    }

    unsafe fn uninstall_irq(&self, dev_irq: u128, handler: *const dyn Device) {
        if dev_irq >= self.irq_ext.len() as u128 {
            return;
        }

        let irq = &self.irq_ext[dev_irq as usize];
        let SocIrqParent::Device(irqctl) = &irq.irqctl else {
            // We shouldn't even be able to get here, since you can't install interrupts like this.
            unreachable!();
        };
        // SAFETY: forwarded from the caller of `Bus::uninstall_irq`, same contract.
        unsafe {
            irqctl
                .irqctl_base()
                .uninstall_irq(irq.vector, dev_irq, handler);
        };
        irqctl.set_irq_in_enabled(irq.vector, false);
    }
}

/// A mapped range from an [`MmioBus`].
pub struct MmioMapping {
    vaddr: usize,
    size: usize,
}

impl MmioMapping {
    /// # Safety
    /// While it is safe to map any arbitrary memory, accessing it can be unsafe.
    /// Therefor, this function is safe but the type only ever returns raw pointers, marking the access as *unsafe*.
    ///
    /// This type only serves to help map physical memory-mapped I/O.
    /// It would be unsafe to dereference the output [`Self::vaddr`] if the caller does not guarantee that the memory still exists.
    /// Therefor, this type should not be used directly for e.g. hot-swappable memory or devices.
    pub fn new(paddr: usize, size: usize) -> EResult<Self> {
        let phys_page_start = paddr - paddr % PAGE_SIZE as usize;
        let phys_page_end = (paddr + size).div_ceil(PAGE_SIZE as usize) * PAGE_SIZE as usize;
        // SAFETY: See function documentation.
        let page_start = unsafe {
            kernel_mm().map(
                phys_page_end - phys_page_start,
                0,
                map::SHARED,
                prot::IO | prot::READ | prot::WRITE,
                Some(Mapping {
                    offset: 0,
                    object: Arc::try_new(RawMemory::new(
                        phys_page_start,
                        phys_page_end - phys_page_start,
                    ))?,
                }),
            )?
        };
        let vaddr = page_start + (paddr - phys_page_start);
        Ok(Self { vaddr, size })
    }

    pub const fn vaddr(&self) -> *mut () {
        self.vaddr as *mut ()
    }

    pub const fn size(&self) -> usize {
        self.size
    }
}

impl Drop for MmioMapping {
    fn drop(&mut self) {
        let page_start = self.vaddr - self.vaddr % PAGE_SIZE as usize;
        let page_end = (self.vaddr + self.size).div_ceil(PAGE_SIZE as usize) * PAGE_SIZE as usize;
        // SAFETY: This mapping is made by a private constructor.
        unsafe {
            if let Err(x) = kernel_mm().unmap(page_start..page_end) {
                logkf!(LogLevel::Error, "MmioMapping unmap failed: {}", x);
            }
        }
    }
}

/// Something that owns an [`MmioMapping`].
pub trait HasMmioMapping {
    fn mmio_mapping(&self) -> &MmioMapping;
}

impl HasMmioMapping for MmioMapping {
    fn mmio_mapping(&self) -> &MmioMapping {
        self
    }
}

impl<T> HasMmioMapping for T
where
    T: Deref<Target = MmioMapping>,
{
    fn mmio_mapping(&self) -> &MmioMapping {
        Deref::deref(self)
    }
}

/// A mapped (sub-)range from an [`MmioMapping`].
pub struct MmioStruct<T: Sized, M: HasMmioMapping = MmioMapping> {
    mapping: M,
    marker: PhantomData<T>,
}

impl<T: Sized, M: HasMmioMapping> MmioStruct<T, M> {
    /// Check the size and alignment and make a wrapper around `mapping` that implements [`Deref`].
    ///
    /// # Safety
    /// Unlike [`MmioMapping`], this type implements [`Deref`] and because the pointer from [`MmioMapping`]
    /// is unchecked, it is unsafe to construct this type.
    /// The caller also promises that [`HasMmioMapping::mmio_mapping`] always returns the same borrow.
    pub unsafe fn new(mapping: M) -> EResult<Self> {
        if size_of::<T>() > mapping.mmio_mapping().size {
            logkf!(
                LogLevel::Error,
                "Mapping ({} bytes) is too small for MmioStruct<{}> ({} bytes)",
                mapping.mmio_mapping().size,
                type_name::<T>(),
                size_of::<T>()
            );
            return Err(Errno::EIO);
        } else if mapping.mmio_mapping().vaddr % align_of::<T>() != 0 {
            logkf!(
                LogLevel::Error,
                "Mapping 0x{:x} is misaligned for MmioStruct<{}> (aligned to {} bytes)",
                mapping.mmio_mapping().vaddr,
                type_name::<T>(),
                align_of::<T>()
            );
            return Err(Errno::EIO);
        }
        Ok(Self {
            mapping,
            marker: PhantomData,
        })
    }
}

impl<T: Sized, M: HasMmioMapping> Deref for MmioStruct<T, M> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.mapping.mmio_mapping().vaddr() as *const T) }
    }
}

impl<T: Sized, M: HasMmioMapping> DerefMut for MmioStruct<T, M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.mapping.mmio_mapping().vaddr() as *mut T) }
    }
}
