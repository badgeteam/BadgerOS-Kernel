// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{
    bindings::{error::EResult, log::LogLevel},
    dev2::{
        Device,
        bus::{
            Bus,
            pci::{
                PciBus,
                addr::{PciAddr, PciIrq, PciPAddr},
                cfg::{self, PciReg, PciRegInfo},
            },
        },
        registry,
    },
    mem::pmm::PAddrr,
};

use alloc::sync::Arc;

pub trait PciCtlDevice: Device {
    /// Whether this is a PCIe controller, (as opposed to a PCI controller).
    fn is_pcie(&self) -> bool;

    /// Get the lowest and highest bus numbers (both inclusive).
    fn bus_range(&self) -> (u8, u8);

    /// Read a configuration space register.
    fn config_read_reg(&self, addr: PciAddr, regno: u8) -> EResult<u32>;

    /// Write a configuration space register.
    ///
    /// # Safety
    /// Writing to PCI configuration register is just like any other MMIO registers;
    /// writing the wrong thing to the wrong one can lead to undefined behaviour.
    unsafe fn config_write_reg(&self, addr: PciAddr, regno: u8, value: u32) -> EResult<()>;

    /// Install the handler for an interrupt.
    ///
    /// # Safety
    /// The caller promises to remove the handler with [`Self::uninstall_irq()`] before it becomes invalid.
    ///
    /// The caller promises that the handler is a valid [`Device`] object.
    unsafe fn install_irq(
        &self,
        addr: PciAddr,
        irq: PciIrq,
        device: *const dyn Device,
    ) -> EResult<()>;

    /// Get the CPU physical address for a PCI physical address;
    /// a PCI address combined with address space type, attributes and offset.
    /// Used by the helper functions to map PCI BARs.
    fn get_cpu_paddr(&self, pci_paddr: PciPAddr) -> Option<PAddrr>;

    /// Remove the handler for an interrupt.
    ///
    /// # Safety
    /// The caller promises that the handler is a valid [`Device`] object.
    unsafe fn uninstall_irq(&self, addr: PciAddr, irq: PciIrq, device: *const dyn Device);
}

impl dyn PciCtlDevice {
    fn enumerate_func(self: &Arc<Self>, addr: PciAddr) -> EResult<()> {
        let htype = self.config_read(addr, cfg::common::HDR_TYPE)?;
        if htype & 0x7f != 0 {
            return Ok(());
        }

        let res = try {
            let bus = PciBus::new(self.clone(), addr, None)?;
            registry::register_bus(bus.clone())?;
            bus
        };
        match res {
            Err(x) => {
                logkf!(
                    LogLevel::Warning,
                    "{}: failed to create bus for {}: {}",
                    self,
                    addr,
                    x
                );
            }
            Ok(ref bus) => {
                logkf!(LogLevel::Info, "Added {} as bus {}", &bus, bus.id());
                logkf!(LogLevel::Info, "  -> class {}", bus.classcode);
            }
        }

        res.map(|_| ())
    }

    fn enumerate_device(self: &Arc<Self>, bus: u8, dev: u8) -> EResult<()> {
        let htype = self.config_read(PciAddr::new(bus, dev, 0), cfg::common::HDR_TYPE)?;
        self.enumerate_func(PciAddr::new(bus, dev, 0))?;

        if htype & 0x80 != 0 {
            for func in 1..8 {
                let vendor = self.config_read(PciAddr::new(bus, dev, func), cfg::common::VENDOR)?;
                if vendor == 0xffff {
                    break;
                }
                self.enumerate_func(PciAddr::new(bus, dev, func))?;
            }
        }

        Ok(())
    }

    /// Enumerate for PCI devices and register their buses.
    ///
    /// # Safety
    /// To avoid duplicate device buses, this function must be called exactly once at the end of the device's initialization code.
    pub unsafe fn enumerate(self: &Arc<Self>) {
        let (bus_start, bus_end) = self.bus_range();

        let mut errors = 0;
        for bus in bus_start..=bus_end {
            for dev in 0..32 {
                if self.enumerate_device(bus, dev).is_err() {
                    errors += 1;
                }
            }
        }

        if errors > 0 {
            logkf!(
                LogLevel::Warning,
                "{} devices partially/not enumerated due to errors",
                errors
            );
        }
    }

    /// Read (sub-)register from configuration space.
    #[inline]
    pub fn config_read<T: PciReg>(&self, addr: PciAddr, subreg: PciRegInfo<T>) -> EResult<T> {
        let reg = (subreg.offset / 4) as u8;
        let byte = subreg.offset % 4;

        let raw = self.config_read_reg(addr, reg)?;
        let mask = 1u32
            .wrapping_shl(8 * size_of::<T::Prim>() as u32)
            .wrapping_sub(1);
        let value = (raw >> (byte * 8)) & mask;

        Ok(T::from(num::cast(value).unwrap()))
    }

    /// Write (sub-)register to configuration space;
    /// will zero other sub-registers if less than 32-bits wide.
    ///
    /// # Safety
    /// Writing to PCI configuration register is just like any other MMIO registers;
    /// writing the wrong thing to the wrong one can lead to undefined behaviour.
    #[inline]
    pub unsafe fn config_write<T: PciReg>(
        &self,
        addr: PciAddr,
        subreg: PciRegInfo<T>,
        value: T,
    ) -> EResult<()> {
        let reg = (subreg.offset / 4) as u8;
        let byte = subreg.offset % 4;

        let value = num::cast::<T::Prim, u32>(value.into()).unwrap();
        let new = value << (8 * byte);

        unsafe { self.config_write_reg(addr, reg, new) }
    }

    /// Read-modify-write (sub-)register to configuration space.
    ///
    /// # Safety
    /// Writing to PCI configuration register is just like any other MMIO registers;
    /// writing the wrong thing to the wrong one can lead to undefined behaviour.
    #[inline]
    pub unsafe fn config_rmw<T: PciReg>(
        &self,
        addr: PciAddr,
        subreg: PciRegInfo<T>,
        value: T,
    ) -> EResult<()> {
        let reg = (subreg.offset / 4) as u8;
        let byte = subreg.offset % 4;

        let raw = self.config_read_reg(addr, reg)?;
        let mask = 1u32
            .wrapping_shl(8 * size_of::<T::Prim>() as u32)
            .wrapping_sub(1);

        let value = num::cast::<T::Prim, u32>(value.into()).unwrap();
        let new = raw & !(mask << (8 * byte)) | value << (8 * byte);

        unsafe { self.config_write_reg(addr, reg, new) }
    }
}

pub const fn cam_addr(bdf: PciAddr, regno: u8) -> usize {
    (bdf.0 as usize) << 8 | (regno as usize) << 2
}

pub const fn ecam_addr(bdf: PciAddr, regno: u8) -> usize {
    (bdf.0 as usize) << 12 | (regno as usize) << 2
}

pub const fn cam_ecam_addr(bdf: PciAddr, regno: u8, is_ecam: bool) -> usize {
    if is_ecam {
        ecam_addr(bdf, regno)
    } else {
        cam_addr(bdf, regno)
    }
}
