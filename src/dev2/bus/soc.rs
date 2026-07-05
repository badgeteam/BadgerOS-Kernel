// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    any::type_name,
    fmt::Display,
    marker::PhantomData,
    ops::{Deref, DerefMut, Range},
};

#[cfg(feature = "dtb")]
use alloc::vec::Vec;
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
#[cfg(feature = "dtb")]
use crate::{
    cpu::PhysCpuID,
    dev2::{self, dtb::DeviceNode, registry},
    kernel::smp,
};

use super::BusBase;

#[derive(Clone)]
pub enum SocIrqParent {
    /// Parent interrupt controller is the CPU interrupt controller.
    Cpu(u32),
    /// Parent interrupt controller is a device.
    Device(Arc<dyn IrqCtlDevice>),
}

/// Interrupt route for [`SocBus`].
#[derive(Clone)]
pub struct SocIrqExt {
    /// Parent inerrupt controller.
    pub irqctl: SocIrqParent,
    /// Interrupt vector of the interrupt controller.
    pub vector: u128,
}

/// Interrupt mapentry for [`SocIrqMap`].
pub struct SocIrqMapEntry {
    pub dev_addr: u128,
    pub dev_irq: u128,
    pub target: Arc<dyn IrqCtlDevice>,
    pub target_irq: u128,
}

/// Interrupt map for [`SocBus`].
pub struct SocIrqMap {
    pub addr_mask: u128,
    pub vector_mask: u128,
    pub map: Box<[SocIrqMapEntry]>,
}

/// System-on-chip memory-mapped I/O bus.
pub struct SocBus {
    /// Base bus struct.
    base: BusBase,
    /// Associated DTB node, if any.
    dtb_node: Option<&'static DtbNode>,
    /// Physical addresses in this MMIO bus.
    paddr: Box<[Range<PAddrr>]>,
    /// Extended interrupts.
    irq_ext: Box<[SocIrqExt]>,
    /// Interrupt map.
    irq_map: Option<SocIrqMap>,
}

impl SocBus {
    pub fn new(
        dtb_node: Option<&'static DtbNode>,
        paddr: Box<[Range<PAddrr>]>,
        irq_ext: Box<[SocIrqExt]>,
        irq_map: Option<SocIrqMap>,
    ) -> Self {
        Self {
            base: BusBase::new(),
            dtb_node,
            paddr,
            irq_ext,
            irq_map,
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

    pub fn irq_map(&self) -> Option<&SocIrqMap> {
        self.irq_map.as_ref()
    }

    /// Install the given child device interrupt on the parent interrupt controller.
    ///
    /// # Safety
    /// The caller promises to remove the handler with [`Self::map_uninstall_irq()`] before it becomes invalid.
    ///
    /// The caller promises that the handler is a valid [`Device`] object.
    pub unsafe fn map_install_irq(
        &self,
        dev_addr: u128,
        dev_irq: u128,
        handler: *const dyn Device,
    ) -> EResult<()> {
        let Some(map) = &self.irq_map else {
            return Err(Errno::EINVAL);
        };
        let addr = dev_addr & map.addr_mask;
        let irq = dev_irq & map.vector_mask;

        for entry in &map.map {
            if entry.dev_addr == addr && entry.dev_irq == irq {
                // SAFETY: Same preconditions.
                unsafe {
                    entry
                        .target
                        .install_irq(entry.target_irq, dev_irq, handler)?
                };
                return Ok(());
            }
        }

        Err(Errno::ENOENT)
    }

    /// Uninstall the given child device interrupt from the parent interrupt controller.
    ///
    /// # Safety
    /// The caller promises that the handler is a valid [`Device`] object.
    pub unsafe fn map_uninstall_irq(
        &self,
        dev_addr: u128,
        dev_irq: u128,
        handler: *const dyn Device,
    ) {
        let Some(map) = &self.irq_map else {
            return;
        };
        let dev_addr = dev_addr & map.addr_mask;

        for entry in &map.map {
            if entry.dev_addr == dev_addr && entry.dev_irq == dev_irq {
                // SAFETY: Same preconditions.
                unsafe {
                    entry
                        .target
                        .uninstall_irq(entry.target_irq, dev_irq, handler)
                };
                return;
            }
        }
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
        unsafe { irqctl.install_irq(irq.vector, dev_irq, handler) }
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
        unsafe { irqctl.uninstall_irq(irq.vector, dev_irq, handler) };
    }
}

impl Display for SocBus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(node) = self.dtb_node {
            Display::fmt(node, f)
        } else {
            f.write_fmt(format_args!("SocBus {}", self.id()))
        }
    }
}

#[cfg(feature = "dtb")]
impl SocBus {
    /// Bus factory for [`dev2::dtb::probe()`] the generates [`SocBus`] instances.
    pub unsafe fn factory(node: DeviceNode) -> EResult<Arc<dyn Bus>> {
        fn get_parent(node: &'static DtbNode) -> EResult<Option<SocIrqParent>> {
            let cpus = dev2::dtb::get().node("cpus").unwrap();

            if let Some(cpu) = node.parent()
                && let Some(cpu_parent) = cpu.parent()
                && core::ptr::addr_eq(cpu_parent, cpus)
            {
                // This node is a CPU interrupt controller.
                let cpuid = cpu.prop_uint("reg").ok_or(Errno::ENOENT)? as PhysCpuID;
                if let Some(idx) = smp::by_phys_id(cpuid) {
                    Ok(Some(SocIrqParent::Cpu(idx)))
                } else {
                    // Non-usable CPU; ignored.
                    Ok(None)
                }
            } else if let Some(irq_bus) = dev2::registry::bus_by_node(node) {
                // This node is a DTB device.
                if let Some(device) = irq_bus.owner() {
                    if let Some(irqctl) = device.try_as_arc() {
                        Ok(Some(SocIrqParent::Device(irqctl)))
                    } else {
                        Err(Errno::EINVAL)
                    }
                } else {
                    Err(Errno::EAGAIN)
                }
            } else {
                // Cannot find this device.
                Err(Errno::ENOENT)
            }
        }

        // Resolve interrupt parents.
        let mut irq_ext = Vec::new();
        for dtb_irq in node.irq.into_iter() {
            match get_parent(dtb_irq.parent) {
                Ok(Some(irqctl)) => irq_ext.push(SocIrqExt {
                    irqctl,
                    vector: dtb_irq.vector,
                }),
                Ok(None) => (),
                Err(x) => return Err(x),
            }
        }

        let irq_map;
        if let Some(raw) = node.irq_map {
            let mut map = Vec::new();
            for entry in raw.map {
                map.try_reserve(1)?;
                map.push(SocIrqMapEntry {
                    dev_addr: entry.addr,
                    dev_irq: entry.irq,
                    target: registry::bus_by_node(entry.target.parent)
                        .ok_or(Errno::EAGAIN)?
                        .owner()
                        .ok_or(Errno::ENODEV)?
                        .try_as_arc()
                        .ok_or(Errno::EINVAL)?,
                    target_irq: entry.target.vector,
                });
            }
            irq_map = Some(SocIrqMap {
                addr_mask: raw.addr_mask,
                vector_mask: raw.vector_mask,
                map: map.into_boxed_slice(),
            });
        } else {
            irq_map = None;
        }

        let bus = Arc::try_new(SocBus::new(
            Some(node.node),
            node.reg
                .into_iter()
                .map(|x| x.start as PAddrr..x.end as PAddrr)
                .collect(),
            irq_ext.into_boxed_slice(),
            irq_map,
        ))?;

        Ok(bus)
    }
}

/// A mapped range from an [`SocBus`].
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
