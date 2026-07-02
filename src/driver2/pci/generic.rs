#[cfg(target_arch = "x86_64")]
use core::arch::asm;
use core::fmt::Display;

use alloc::{boxed::Box, sync::Arc};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    dev2::{
        self, Device, DeviceBase,
        bus::{
            Bus,
            soc::{MmioMapping, SocBus},
        },
        class::pcictl::{
            PciCtlDevice,
            addr::{PciAddr, PciIrq, PciPAddr, PciSeg},
            cam_ecam_addr,
        },
        driver::Driver,
    },
    device_get_trait_vtable,
    mem::pmm::PAddrr,
};

#[derive(Clone, Copy)]
pub struct PciBarRange {
    pub pci_paddr: PciPAddr,
    pub cpu_paddr: PAddrr,
    pub size: u64,
}

pub struct PciCtlGeneric {
    base: DeviceBase,
    bus: Arc<SocBus>,
    ranges: Box<[PciBarRange]>,
    bus_start: u8,
    bus_end: u8,
    config: MmioMapping,
    is_pcie: bool,
}

impl PciCtlGeneric {
    pub unsafe fn new(
        base: DeviceBase,
        bus: Arc<SocBus>,
        ranges: Box<[PciBarRange]>,
        bus_start: u8,
        bus_end: u8,
        is_pcie: bool,
    ) -> EResult<Arc<Self>> {
        let config = bus.map(0)?;
        if is_pcie {
            if config.size() < 0x1000 * (bus_end as usize + 1) {
                logkf!(LogLevel::Error, "PCIe configuration space is too small");
                return Err(Errno::EINVAL);
            }
        } else {
            if config.size() < 0x100 * (bus_end as usize + 1) {
                logkf!(LogLevel::Error, "PCI configuration space is too small");
                return Err(Errno::EINVAL);
            }
        }

        let this = Arc::try_new(Self {
            base,
            bus: bus.clone(),
            ranges,
            bus_start,
            bus_end,
            config,
            is_pcie,
        })?;
        <dyn Bus>::claim(&*bus, Arc::<PciCtlGeneric>::downgrade(&this))?;

        Ok(this)
    }
}

impl Device for PciCtlGeneric {
    fn base(&self) -> &DeviceBase {
        &self.base
    }

    fn interrupt(&self, _id: u128) -> bool {
        unreachable!()
    }

    device_get_trait_vtable!(PciCtlDevice);
}

impl PciCtlDevice for PciCtlGeneric {
    fn is_pcie(&self) -> bool {
        self.is_pcie
    }

    fn bus_range(&self) -> (u8, u8) {
        (self.bus_start, self.bus_end)
    }

    fn get_cpu_paddr(&self, pci_paddr: PciPAddr) -> Option<PAddrr> {
        for entry in &self.ranges {
            if pci_paddr.seg() != entry.pci_paddr.seg()
                // 64-bit BARs can be placed in the MEM32 area if there is enough space.
                && pci_paddr.seg() != PciSeg::Mem64
                && entry.pci_paddr.seg() != PciSeg::Mem32
            {
                continue;
            }
            let mask = entry.size.wrapping_neg();
            if pci_paddr.seg_addr() & mask != entry.pci_paddr.seg_addr() {
                continue;
            }
            let offset = pci_paddr.seg_addr() & !mask;
            return Some(entry.cpu_paddr + offset as PAddrr);
        }

        None
    }

    fn config_read_reg(&self, addr: PciAddr, regno: u8) -> EResult<u32> {
        let addr = cam_ecam_addr(addr, regno, self.is_pcie);
        if addr >= self.config.size() {
            return Err(Errno::ERANGE);
        }

        let res;
        unsafe {
            // SAFETY: We checked the bounds of this pointer earlier.
            let ptr = self.config.vaddr().byte_add(addr) as *mut u32;

            // On some older x86_64 CPUs, you can only access configuration space through EAX.
            #[cfg(target_arch = "x86_64")]
            asm!(
                "mov eax, dword ptr [{}]",
                in(reg) addr,
                out("eax") res
            );

            // Other architectures can use normal memory accesses.
            #[cfg(not(target_arch = "x86_64"))]
            {
                res = ptr.read_volatile();
            }
        }

        Ok(res)
    }

    unsafe fn config_write_reg(&self, addr: PciAddr, regno: u8, value: u32) -> EResult<()> {
        let addr = cam_ecam_addr(addr, regno, self.is_pcie);
        if addr >= self.config.size() {
            return Err(Errno::ERANGE);
        }

        unsafe {
            // SAFETY: We checked the bounds of this pointer earlier.
            let ptr = self.config.vaddr().byte_add(addr) as *mut u32;

            // On some older x86_64 CPUs, you can only access configuration space through EAX.
            #[cfg(target_arch = "x86_64")]
            asm!(
                "mov dword ptr [{}], eax",
                in(reg) addr,
                in("eax") value
            );

            // Other architectures can use normal memory accesses.
            #[cfg(not(target_arch = "x86_64"))]
            {
                ptr.write_volatile(value);
            }
        }

        Ok(())
    }

    unsafe fn install_irq(
        &self,
        addr: PciAddr,
        irq: PciIrq,
        device: *const dyn Device,
    ) -> EResult<()> {
        let dev_addr = PciPAddr::new_config(addr, 0).into();
        // SAFETY: Same preconditions.
        unsafe { self.bus.map_install_irq(dev_addr, irq as u128, device) }
    }

    unsafe fn uninstall_irq(&self, addr: PciAddr, irq: PciIrq, device: *const dyn Device) {
        let dev_addr = PciPAddr::new_config(addr, 0).into();
        // SAFETY: Same preconditions.
        unsafe { self.bus.map_uninstall_irq(dev_addr, irq as u128, device) }
    }
}

impl Display for PciCtlGeneric {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.bus.fmt(f)
    }
}

pub struct PciGenericDriver;

impl Driver for PciGenericDriver {
    fn name(&self) -> &str {
        "pci-generic"
    }

    fn match_(&self, bus: &dyn Bus) -> bool {
        let Some(node) = bus.dtb_node() else {
            return false;
        };

        node.is_compatible_any(&["pci-host-ecam-generic", "pci-host-cam-generic"])
    }

    unsafe fn probe(&self, bus: Arc<dyn Bus>) -> EResult<Arc<dyn Device>> {
        let Some(node) = bus.dtb_node() else {
            return Err(Errno::EINVAL);
        };
        let bus = Arc::downcast::<SocBus>(bus).unwrap();

        let Some(bus_range_prop) = node.prop("bus-range") else {
            logkf!(LogLevel::Error, "{}: missing bus-range", node);
            return Err(Errno::EINVAL);
        };
        if bus_range_prop.blob.len() != 8 {
            logkf!(LogLevel::Error, "{}: malformed bus-range", node);
            return Err(Errno::EINVAL);
        }

        let bus_start = bus_range_prop.read_cell(0).unwrap() as u8;
        let bus_end = bus_range_prop.read_cell(1).unwrap() as u8;

        let ranges = dev2::dtb::parse_ranges(node)?
            .iter()
            .map(|x| PciBarRange {
                pci_paddr: x.child_addr.into(),
                cpu_paddr: x.parent_addr as usize,
                size: x.size as u64,
            })
            .collect();
        let dev: Arc<dyn PciCtlDevice> = unsafe {
            PciCtlGeneric::new(
                DeviceBase::new(),
                bus,
                ranges,
                bus_start,
                bus_end,
                node.is_compatible("pci-host-ecam-generic"),
            )?
        };
        // SAFETY: Enumeration should be ran at the end of device initialization.
        unsafe { dev.enumerate() };

        Ok(dev)
    }
}
