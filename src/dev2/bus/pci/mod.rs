// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::fmt::Display;

use addr::{PciAddr, PciIrq, PciPAddr};
use alloc::sync::Arc;
use bar::BarInfo;
use classcode::ClassCode;
use dtb::DtbNode;

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    dev2::{Device, class::pcictl::PciCtlDevice},
};

use super::{Bus, BusBase};

pub mod addr;
pub mod bar;
pub mod cfg;
pub mod classcode;

/// PCI or PCI express bus.
pub struct PciBus {
    /// Base bus struct.
    base: BusBase,
    /// Associated DTB node, if any.
    dtb_node: Option<&'static DtbNode>,
    /// PCI address.
    pub addr: PciAddr,
    /// PCI vendor ID.
    pub vendor: u16,
    /// PCI device ID.
    pub device: u16,
    /// Class code.
    pub classcode: ClassCode,
    /// Parent PCI controller.
    pub ctrl: Arc<dyn PciCtlDevice>,
}

impl PciBus {
    pub fn new(
        ctrl: Arc<dyn PciCtlDevice>,
        addr: PciAddr,
        dtb_node: Option<&'static DtbNode>,
    ) -> EResult<Arc<Self>> {
        let device = ctrl.config_read(addr, cfg::common::DEVICE)?;
        let vendor = ctrl.config_read(addr, cfg::common::VENDOR)?;
        let progif = ctrl.config_read(addr, cfg::common::PROGIF)?;
        let subclass = ctrl.config_read(addr, cfg::common::SUBCLASS)?;
        let baseclass = ctrl.config_read(addr, cfg::common::BASECLASS)?;

        let this = Arc::try_new(Self {
            base: BusBase::new(),
            dtb_node,
            addr,
            vendor,
            device,
            classcode: ClassCode {
                baseclass,
                subclass,
                progif,
            },
            ctrl,
        })?;

        Ok(this)
    }

    /// Install the handler for an interrupt.
    /// Convenience function over directly calling [`Bus::install_irq()`].
    ///
    /// # Safety
    /// The caller promises to remove the handler with [`Self::uninstall_pci_irq()`] before it becomes invalid.
    ///
    /// The caller promises that the handler is a valid [`Device`] object.
    pub unsafe fn install_pci_irq(&self, irq: PciIrq, device: *const dyn Device) -> EResult<()> {
        unsafe { self.ctrl.install_irq(self.addr, irq, device) }
    }

    /// Remove the handler for an interrupt.
    /// Convenience function over directly calling [`Bus::uninstall_irq()`].
    ///
    /// # Safety
    /// The caller promises that the handler is a valid [`Device`] object.
    pub unsafe fn uninstall_pci_irq(&self, irq: PciIrq, device: *const dyn Device) {
        unsafe { self.ctrl.uninstall_irq(self.addr, irq, device) }
    }

    /// Get information about this function's BARs.
    ///
    /// # Safety
    /// Some PCI devices may have been initialized by the firmware; probing for BAR info modifies the addresses temporarily,
    /// which may interfere with firmware-configured or even firmware-owned devices.
    pub unsafe fn bar_info(&self) -> EResult<[Option<BarInfo>; 6]> {
        let mut raw = [0; 6];
        for i in 0u8..6u8 {
            raw[i as usize] = self.ctrl.config_read_reg(self.addr, 4 + i)?;
        }
        let mut masked = [0; 6];
        for i in 0u8..6u8 {
            unsafe {
                self.ctrl.config_write_reg(self.addr, 4 + i, u32::MAX)?;
                let tmp = self.ctrl.config_read_reg(self.addr, 4 + i)?;
                masked[i as usize] = tmp & !(tmp << 1);
                self.ctrl
                    .config_write_reg(self.addr, 4 + i, raw[i as usize])?;
            }
        }
        let mut res = [const { None }; 6];

        let mut i = 0;
        while i < 6 {
            if raw[i] & cfg::BAR_FLAG_IO != 0 {
                res[i] = Some(BarInfo {
                    seg_addr: (raw[i] & cfg::BAR_IO_ADDR_MASK) as u64,
                    cpu_paddr: 0,
                    size: (masked[i] & cfg::BAR_IO_ADDR_MASK) as usize,
                    type_: bar::BarType::IO,
                    prefetch: false,
                });
                i += 1;
            } else if raw[i] & cfg::BAR_FLAG_64BIT != 0 {
                if i == 5 {
                    logkf!(
                        LogLevel::Error,
                        "{}: BAR5 cannot be Mem64, but proports to be",
                        self
                    );
                    return Err(Errno::EINVAL);
                }
                let addr = raw[i] as u64 | (raw[i + 1] as u64) << 32;
                res[i] = Some(BarInfo {
                    seg_addr: addr & cfg::BAR_MEM64_ADDR_MASK,
                    cpu_paddr: 0,
                    size: (masked[i] & cfg::BAR_MEM32_ADDR_MASK) as usize,
                    type_: bar::BarType::Mem64,
                    prefetch: raw[i] & cfg::BAR_FLAG_PREFETCH != 0,
                });
                i += 2;
            } else {
                res[i] = Some(BarInfo {
                    seg_addr: (raw[i] & cfg::BAR_MEM32_ADDR_MASK) as u64,
                    cpu_paddr: 0,
                    size: (masked[i] & cfg::BAR_MEM32_ADDR_MASK) as usize,
                    type_: bar::BarType::Mem32,
                    prefetch: raw[i] & cfg::BAR_FLAG_PREFETCH != 0,
                });
                i += 1;
            }
        }

        for i in 0..6 {
            let Some(bar) = &mut res[i] else {
                continue;
            };

            let pci_paddr = PciPAddr::new(
                false,
                bar.prefetch,
                false,
                bar.type_.into(),
                self.addr,
                4 + i as u8,
                bar.seg_addr,
            );
            match self.ctrl.get_cpu_paddr(pci_paddr) {
                Some(paddr) => bar.cpu_paddr = paddr,
                None if bar.seg_addr == 0 => {
                    res[i] = None;
                }
                None => {
                    logkf!(
                        LogLevel::Error,
                        "{}: Invalid address for {:?} BAR{}, 0x{:x}",
                        self,
                        bar.type_,
                        i,
                        bar.seg_addr
                    );
                    return Err(Errno::EINVAL);
                }
            }
        }

        Ok(res)
    }
}

impl Bus for PciBus {
    fn base(&self) -> &BusBase {
        &self.base
    }

    fn parent_device(&self) -> Option<Arc<dyn Device>> {
        Some(self.ctrl.clone())
    }

    unsafe fn install_irq(&self, irq: u128, device: *const dyn Device) -> EResult<()> {
        unsafe { self.install_pci_irq(PciIrq::try_from(irq).map_err(|_| Errno::EINVAL)?, device) }
    }

    unsafe fn uninstall_irq(&self, irq: u128, device: *const dyn Device) {
        if let Ok(irq) = PciIrq::try_from(irq) {
            unsafe {
                self.uninstall_pci_irq(irq, device);
            }
        }
    }

    fn dtb_node(&self) -> Option<&'static DtbNode> {
        self.dtb_node
    }
}

impl Display for PciBus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}/{}", self.ctrl, self.addr))
    }
}
