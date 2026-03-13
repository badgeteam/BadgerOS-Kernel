use core::num::NonZero;

use crate::bindings::raw;

/// Memory-mapped I/O address.
#[derive(Debug, Clone, Copy)]
pub struct MmioAddr {
    /// Physical base address.
    pub paddr: u64,
    /// Virtual base address, if mapped.
    pub vaddr: Option<NonZero<usize>>,
    /// Size of the memory region.
    pub size: usize,
}

impl Into<raw::dev_mmio_addr_t> for MmioAddr {
    fn into(self) -> raw::dev_mmio_addr_t {
        raw::dev_mmio_addr_t {
            paddr: self.paddr,
            vaddr: self.vaddr.map_or(0, |v| v.get()),
            size: self.size,
        }
    }
}

impl From<raw::dev_mmio_addr_t> for MmioAddr {
    fn from(addr: raw::dev_mmio_addr_t) -> Self {
        MmioAddr {
            paddr: addr.paddr,
            vaddr: NonZero::new(addr.vaddr),
            size: addr.size,
        }
    }
}

/// PCI or PCIe address.
#[derive(Debug, Clone, Copy)]
pub struct PciAddr {
    /// Bus number (0-255).
    pub bus: u8,
    /// Device number (0-31).
    pub dev: u8,
    /// Function number (0-7).
    pub func: u8,
}

impl Into<raw::dev_pci_addr_t> for PciAddr {
    fn into(self) -> raw::dev_pci_addr_t {
        let mut tmp = raw::dev_pci_addr_t { val: 0 };
        unsafe {
            tmp.__bindgen_anon_1.set_bus(self.bus);
            tmp.__bindgen_anon_1.set_dev(self.dev);
            tmp.__bindgen_anon_1.set_func(self.func);
        }
        tmp
    }
}

impl From<raw::dev_pci_addr_t> for PciAddr {
    fn from(addr: raw::dev_pci_addr_t) -> Self {
        PciAddr {
            bus: unsafe { addr.__bindgen_anon_1.bus() },
            dev: unsafe { addr.__bindgen_anon_1.dev() },
            func: unsafe { addr.__bindgen_anon_1.func() },
        }
    }
}

/// I2C address.
#[derive(Debug, Clone, Copy)]
pub struct I2cAddr(u16);

/// AHCI drive address.
#[derive(Debug, Clone, Copy)]
pub struct AhciAddr {
    /// Port number (0-31).
    pub port: u8,
    /// Port multiplier port number (0-15).
    pub pmul: Option<u8>,
}

impl Into<raw::dev_ahci_addr_t> for AhciAddr {
    fn into(self) -> raw::dev_ahci_addr_t {
        raw::dev_ahci_addr_t {
            port: self.port,
            pmul_port: self.pmul.unwrap_or(0),
            pmul: self.pmul.is_some(),
        }
    }
}

impl From<raw::dev_ahci_addr_t> for AhciAddr {
    fn from(addr: raw::dev_ahci_addr_t) -> Self {
        AhciAddr {
            port: addr.port,
            pmul: addr.pmul.then_some(addr.pmul_port),
        }
    }
}

/// Device address; bus is implied by parent device.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum DevAddr {
    Mmio(MmioAddr) = raw::dev_atype_t_DEV_ATYPE_MMIO as u32,
    Pci(PciAddr) = raw::dev_atype_t_DEV_ATYPE_PCI as u32,
    I2c(I2cAddr) = raw::dev_atype_t_DEV_ATYPE_I2C as u32,
    Ahci(AhciAddr) = raw::dev_atype_t_DEV_ATYPE_AHCI as u32,
}

impl Into<raw::dev_addr_t> for DevAddr {
    fn into(self) -> raw::dev_addr_t {
        match self {
            DevAddr::Mmio(addr) => raw::dev_addr_t {
                type_: raw::dev_atype_t_DEV_ATYPE_MMIO,
                __bindgen_anon_1: raw::dev_addr_t__bindgen_ty_1 { mmio: addr.into() },
            },
            DevAddr::Pci(addr) => raw::dev_addr_t {
                type_: raw::dev_atype_t_DEV_ATYPE_PCI,
                __bindgen_anon_1: raw::dev_addr_t__bindgen_ty_1 { pci: addr.into() },
            },
            DevAddr::I2c(addr) => raw::dev_addr_t {
                type_: raw::dev_atype_t_DEV_ATYPE_I2C,
                __bindgen_anon_1: raw::dev_addr_t__bindgen_ty_1 { i2c: addr.0 },
            },
            DevAddr::Ahci(addr) => raw::dev_addr_t {
                type_: raw::dev_atype_t_DEV_ATYPE_AHCI,
                __bindgen_anon_1: raw::dev_addr_t__bindgen_ty_1 { ahci: addr.into() },
            },
        }
    }
}

impl From<raw::dev_addr_t> for DevAddr {
    fn from(addr: raw::dev_addr_t) -> Self {
        unsafe {
            match addr.type_ {
                raw::dev_atype_t_DEV_ATYPE_MMIO => DevAddr::Mmio(addr.__bindgen_anon_1.mmio.into()),
                raw::dev_atype_t_DEV_ATYPE_PCI => DevAddr::Pci(addr.__bindgen_anon_1.pci.into()),
                raw::dev_atype_t_DEV_ATYPE_I2C => DevAddr::I2c(I2cAddr(addr.__bindgen_anon_1.i2c)),
                raw::dev_atype_t_DEV_ATYPE_AHCI => DevAddr::Ahci(addr.__bindgen_anon_1.ahci.into()),
                _ => panic!("Unknown device address type"),
            }
        }
    }
}
