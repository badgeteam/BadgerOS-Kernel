// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::{collections::btree_map::BTreeMap, vec::Vec};

use crate::{
    badgelib::irq::IrqGuard, bindings::error::EResult, cpu::irq, dev2::device::Device,
    kernel::sync::spinlock::Spinlock,
};

/// Base interrupt controller struct; intended for use by implementers of [`IrqCtlDevice`].
pub struct IrqCtlDeviceBase {
    pub mask: u128,
    handlers: Spinlock<BTreeMap<u128, Vec<(u128, *const dyn Device)>>>,
}

// SAFETY: The handler pointers are only dereferenced under the spinlock with interrupts
// disabled, and devices promise to uninstall their handlers before becoming invalid.
unsafe impl Send for IrqCtlDeviceBase {}
unsafe impl Sync for IrqCtlDeviceBase {}

/// An interrupt controller (e.g. I/O APIC, PLIC, PCI(e) controller, etc).
pub trait IrqCtlDevice: Device {
    /// Get the base device struct.
    fn irqctl_base(&self) -> &IrqCtlDeviceBase;

    /// Whether this controller can remap interrupt inputs to outputs dynamically.
    /// If the same input always goes to the same output, this must return false.
    fn can_remap(&self) -> bool;

    /// Change which interrupt output is triggered by an interrupt input.
    /// Invalid on devices that do not advertise [`Self::can_remap`].
    fn remap(&self, in_irq: u128, out_irq: u128) -> EResult<()>;

    /// Change interrupt trigger mode.
    fn irq_trigger_mode(&self, in_irq: u128, is_edge: bool) -> EResult<()>;

    /// Enable or disable an interrupt input line on this controller.
    /// For a PLIC this sets the source priority and per-context enable bit;
    /// for other controllers it manipulates their equivalent enable state.
    fn set_irq_in_enabled(&self, in_irq: u128, enable: bool) -> EResult<()>;
}

impl IrqCtlDeviceBase {
    pub fn new(mask: u128) -> EResult<Self> {
        Ok(Self {
            mask,
            handlers: Spinlock::new(BTreeMap::new()),
        })
    }

    /// Install the handler for an interrupt.
    ///
    /// # Safety
    /// The caller promises to remove the handler with [`Bus::remove_irq`] before it becomes invalid.
    pub(crate) unsafe fn install_irq(
        &self,
        irq_id: u128,
        dev_irq: u128,
        device: *const dyn Device,
    ) -> EResult<()> {
        let irq_id = irq_id & self.mask;
        let _noirq = IrqGuard::new();
        let mut handlers = self.handlers.lock();

        if let Some(irq) = handlers.get_mut(&irq_id) {
            irq.try_reserve(1)?;
            irq.push((dev_irq, device));
        } else {
            let mut vec = Vec::try_with_capacity(1)?;
            vec.push((dev_irq, device));
            handlers.insert(irq_id, vec);
        }

        Ok(())
    }

    /// Remove the handler for an interrupt.
    ///
    /// # Safety
    /// The caller promises that the handler is a valid [`Device`] object.
    pub(crate) unsafe fn uninstall_irq(&self, irq_id: u128, dev_irq: u128, device: *const dyn Device) {
        let irq_id = irq_id & self.mask;
        let _noirq = IrqGuard::new();
        let mut handlers = self.handlers.lock();

        if let Some(irq) = handlers.get_mut(&irq_id) {
            irq.retain(|x| x.0 != dev_irq || !core::ptr::addr_eq(x.1, device));
        }
    }

    /// Run the handler(s) for an interrupt.
    /// Returns whether the interrupt was handled.
    pub(crate) fn run_handlers(&self, irq_id: u128) -> bool {
        let irq_id = irq_id & self.mask;
        debug_assert!(!irq::is_enabled());

        let mut handled = false;
        let handlers = self.handlers.lock_shared();
        if let Some(irq) = handlers.get(&irq_id) {
            for &(dev_irq, device) in irq {
                // SAFETY: The device promises to uninstall its interrupts before it becomes invalid.
                handled |= unsafe { (*device).interrupt(dev_irq) };
            }
        }

        handled
    }
}
