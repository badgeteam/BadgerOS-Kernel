// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

//! Driver for the RISC-V Platform-Level Interrupt Controller (PLIC).
//!
//! The PLIC is a dev2 [`IrqCtlDevice`]: external interrupts (cause 9) are dispatched to
//! it by the arch root, it claims/completes from the current hart's context, and forwards
//! the claimed source to the leaf device's handler. CLINT (timer/IPI) is arch-managed and
//! never reaches here. More than one PLIC may exist; each owns its harts' contexts.

use core::any::Any;

use alloc::{boxed::Box, sync::Arc, vec::Vec};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    dev2::{
        Device, DeviceBase,
        bus::{
            Bus,
            soc::{MmioMapping, SocBus, SocIrqParent},
        },
        class::irqctl::{IrqCtlDevice, IrqCtlDeviceBase},
        driver::Driver,
    },
    device_get_trait_vtable,
    kernel::smp,
};

/// Supervisor external interrupt cause number (PLIC output to a hart).
const RISCV_INT_EXT: u32 = 9;

// PLIC register offsets (see the RISC-V PLIC spec).

/// Per-source priority: `PLIC_PRIO_OFF + source * 4`.
const PLIC_PRIO_OFF: usize = 0x0000;
/// Per-context enable bitmap base.
const fn enable_off(ctx: u32) -> usize {
    0x2000 + ctx as usize * 0x80
}
/// Per-context priority threshold.
const fn thresh_off(ctx: u32) -> usize {
    0x200000 + ctx as usize * 0x1000
}
/// Per-context claim/complete register.
const fn claim_off(ctx: u32) -> usize {
    0x200004 + ctx as usize * 0x1000
}

/// A Platform-Level Interrupt Controller instance.
pub struct RiscvPlic {
    base: DeviceBase,
    irqctl: IrqCtlDeviceBase,
    /// Mapping of the whole PLIC register window.
    mapping: MmioMapping,
    /// Number of interrupt sources (`riscv,ndev`).
    ndev: u32,
    /// PLIC context number to use per SMP index, if this PLIC serves that hart.
    ctx_by_cpu: Box<[Option<u32>]>,
    /// Bus reservation.
    bus: Arc<SocBus>,
}

impl RiscvPlic {
    fn read32(&self, off: usize) -> u32 {
        // SAFETY: `off` is within the mapped PLIC window; MMIO access is volatile.
        unsafe { ((self.mapping.vaddr() as usize + off) as *const u32).read_volatile() }
    }

    fn write32(&self, off: usize, val: u32) {
        // SAFETY: `off` is within the mapped PLIC window; MMIO access is volatile.
        unsafe { ((self.mapping.vaddr() as usize + off) as *mut u32).write_volatile(val) }
    }
}

impl Device for RiscvPlic {
    fn base(&self) -> &DeviceBase {
        &self.base
    }

    /// Dispatched by the arch root for external interrupts: claim, complete, forward.
    fn interrupt(&self, _cause: u128) -> bool {
        let smp_idx = smp::cur_cpu() as usize;
        let Some(Some(ctx_no)) = self.ctx_by_cpu.get(smp_idx).copied() else {
            return false;
        };
        let claim = self.read32(claim_off(ctx_no));
        if claim == 0 {
            // Spurious, or already claimed/handled by another hart.
            return true;
        }
        let handled = self.irqctl.run_handlers(claim as u128);
        // Complete the interrupt regardless so the PLIC can re-arm the source.
        self.write32(claim_off(ctx_no), claim);
        handled
    }

    device_get_trait_vtable!(IrqCtlDevice);
}

impl IrqCtlDevice for RiscvPlic {
    fn irqctl_base(&self) -> &IrqCtlDeviceBase {
        &self.irqctl
    }

    fn can_remap(&self) -> bool {
        false
    }

    fn remap(&self, _in_irq: u128, _out_irq: u128) -> EResult<()> {
        Err(Errno::ENOTSUP)
    }

    fn irq_trigger_mode(&self, _in_irq: u128, _is_edge: bool) -> EResult<()> {
        Err(Errno::ENOTSUP)
    }

    fn set_irq_in_enabled(&self, source: u128, enable: bool) -> EResult<()> {
        let source = source as usize;
        if source == 0 || source as u32 > self.ndev {
            return Err(Errno::EINVAL);
        }
        // A nonzero priority is required for the source to ever fire.
        self.write32(PLIC_PRIO_OFF + source * 4, enable as u32);
        let bit = 1u32 << (source % 32);
        for ctx_no in self.ctx_by_cpu.iter().flatten() {
            let off = enable_off(*ctx_no) + source / 32 * 4;
            let mut bits = self.read32(off);
            if enable {
                bits |= bit;
            } else {
                bits &= !bit;
            }
            self.write32(off, bits);
        }
        Ok(())
    }
}

/// The PLIC driver, registered into the dev2 driver table.
pub struct RiscvPlicDriver;

impl Driver for RiscvPlicDriver {
    fn name(&self) -> &str {
        "riscv-plic"
    }

    fn match_(&self, bus: &dyn Bus) -> bool {
        if (bus as &dyn Any).downcast_ref::<SocBus>().is_none() {
            return false;
        }
        let Some(node) = bus.dtb_node() else {
            return false;
        };
        node.is_compatible_any(&["riscv,plic0", "sifive,plic-1.0.0"])
    }

    unsafe fn probe(&self, bus: Arc<dyn Bus>) -> EResult<Arc<dyn Device>> {
        let bus = Arc::downcast::<SocBus>(bus).unwrap();
        let node = bus.dtb_node().unwrap();
        let base = DeviceBase::new();

        // Map the whole PLIC register window.
        let mapping = bus.map(0)?;

        // PLIC interrupt specifiers are a single cell (the source number).
        if node.irq_cells != Some(1) {
            logkf!(LogLevel::Error, "{}: #interrupt-cells must be 1", node);
            return Err(Errno::EINVAL);
        }
        let ndev = node.prop_u32("riscv,ndev").ok_or(Errno::EINVAL)?;

        // Map each PLIC context (an `interrupts-extended` entry) to an SMP index.
        // Entry order defines the context number; only supervisor-external outputs are used.
        let ncpu = smp::cpu_index_end() as usize;
        let mut ctx_by_cpu: Vec<Option<u32>> = Vec::new();
        ctx_by_cpu.try_reserve(ncpu)?;
        ctx_by_cpu.resize(ncpu, None);

        for (ctx_no, entry) in bus.irq_ext().iter().enumerate() {
            // Only care about S-mode external interrupts here.
            if entry.vector != RISCV_INT_EXT as u128 {
                continue;
            }
            if let SocIrqParent::Cpu(idx) = &entry.irqctl
                && let Some(slot) = ctx_by_cpu.get_mut(*idx as usize)
            {
                *slot = Some(ctx_no as u32);
            }
        }

        let plic = Arc::try_new(RiscvPlic {
            base,
            irqctl: IrqCtlDeviceBase::new(u128::MAX)?,
            mapping,
            ndev,
            ctx_by_cpu: ctx_by_cpu.into_boxed_slice(),
            bus: bus.clone(),
        })?;
        bus.claim(Arc::<RiscvPlic>::downgrade(&plic))?;

        // Mask nothing (threshold 0) on each used context; sources are enabled lazily.
        for ctx_no in plic.ctx_by_cpu.iter().flatten() {
            plic.write32(thresh_off(*ctx_no), 0);
        }

        // Attach to the arch root on each hart we serve. The supervisor external
        // `sie` bit is already enabled per hart at boot (see `arch_cpu_spinup`).
        for (smp_idx, slot) in plic.ctx_by_cpu.iter().enumerate() {
            if slot.is_some() {
                smp::register_ext_irqctl(smp_idx as u32, plic.clone())
                    .expect("Failed to register PLIC as interrupt controller");
            }
        }

        Ok(plic)
    }
}
