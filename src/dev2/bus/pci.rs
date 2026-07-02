// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::fmt::Display;

use alloc::sync::Arc;
use dtb::DtbNode;

use crate::{
    bindings::error::{EResult, Errno},
    dev2::{
        Device,
        class::pcictl::{
            PciCtlDevice,
            addr::{PciAddr, PciIrq},
            cfg,
        },
    },
};

use super::{Bus, BusBase};

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
    /// Programming interface.
    pub progif: u8,
    /// Subclass.
    pub subclass: u8,
    /// Base class.
    pub baseclass: u8,
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
            progif,
            subclass,
            baseclass,
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
    unsafe fn install_pci_irq(&self, irq: PciIrq, device: *const dyn Device) -> EResult<()> {
        unsafe { self.ctrl.install_irq(self.addr, irq, device) }
    }

    /// Remove the handler for an interrupt.
    /// Convenience function over directly calling [`Bus::uninstall_irq()`].
    ///
    /// # Safety
    /// The caller promises that the handler is a valid [`Device`] object.
    unsafe fn uninstall_pci_irq(&self, irq: PciIrq, device: *const dyn Device) {
        unsafe { self.ctrl.uninstall_irq(self.addr, irq, device) }
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
