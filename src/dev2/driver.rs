// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

//! Generic device-tree driver framework: drivers register themselves into a linker
//! section, and [`probe_all`] walks the device tree binding drivers to nodes by
//! their `compatible` string, with deferred probing for dependency ordering.

use core::ops::Range;

use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc, vec::Vec};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    device::dtb::{Dtb, DtbNode},
    mem::pmm::PAddrr,
};

use super::{
    bus::mmio::MmioBus,
    device::{Device, class::irqctl::IrqCtlDevice},
    registry,
};

/// A driver that can bind to device-tree nodes.
pub trait Driver: Sync {
    /// Human-readable driver name (for logging).
    fn name(&self) -> &str;

    /// Whether this driver can drive the given node (usually a `compatible` check).
    fn matches(&self, node: &DtbNode) -> bool;

    /// Instantiate the device for `node`.
    /// Return [`Errno::EAGAIN`] to defer until a later pass (e.g. when an interrupt
    /// parent has not been probed yet).
    fn probe(&self, ctx: &ProbeContext) -> EResult<Arc<dyn Device>>;
}

/// Reference to a registered driver, as stored in the `.dev2_drivers` linker section.
pub type DriverRef = &'static (dyn Driver + Sync);

unsafe extern "C" {
    #[link_name = "__start_dev2_drivers"]
    static DRIVERS_START: u8;
    #[link_name = "__stop_dev2_drivers"]
    static DRIVERS_STOP: u8;
}

/// Register a driver into the `.dev2_drivers` linker section.
/// Use at most once per module; `$drv` must be a `'static` value implementing [`Driver`].
#[macro_export]
macro_rules! register_driver {
    ($drv:expr) => {
        #[used]
        #[unsafe(link_section = ".dev2_drivers")]
        static DEV2_DRIVER_ENTRY: $crate::dev2::driver::DriverRef = &$drv;
    };
}

/// Iterate all registered drivers.
fn registered_drivers() -> &'static [DriverRef] {
    let start = &raw const DRIVERS_START as *const DriverRef;
    let stop = &raw const DRIVERS_STOP as *const DriverRef;
    let len = unsafe { stop.offset_from(start) } as usize;
    unsafe { core::slice::from_raw_parts(start, len) }
}

/// A single parsed interrupt specifier of a device.
pub struct IrqEntry {
    /// The interrupt controller node this entry connects to.
    pub parent: &'static DtbNode,
    /// The interrupt specifier cells (length = parent's `#interrupt-cells`).
    pub cells: Box<[u32]>,
}

/// Context handed to a [`Driver::probe`], exposing the node and resolution helpers.
pub struct ProbeContext<'a> {
    /// The full device tree (for phandle resolution).
    pub dtb: &'static Dtb,
    /// The node being probed.
    pub node: &'static DtbNode,
    /// Interrupt controllers probed so far, keyed by phandle.
    phandle_irqctls: &'a BTreeMap<u32, Arc<dyn IrqCtlDevice>>,
}

impl ProbeContext<'_> {
    /// Parse this node's `reg` into physical address ranges using the parent's
    /// `#address-cells` / `#size-cells`.
    pub fn reg(&self) -> EResult<Box<[Range<PAddrr>]>> {
        let parent = self.node.parent().ok_or(Errno::EINVAL)?;
        let ac = cells(parent, "#address-cells")?;
        let sc = cells(parent, "#size-cells")?;
        let stride = ac + sc;
        let reg = self.node.props.get("reg").ok_or(Errno::EINVAL)?;
        if stride == 0 {
            return Err(Errno::EINVAL);
        }
        let count = reg.cell_count() / stride;
        let mut out = Vec::new();
        out.try_reserve(count)?;
        for i in 0..count {
            let base = i * stride;
            let addr = reg.read_uint_cells(base..base + ac).ok_or(Errno::EINVAL)? as PAddrr;
            let size = reg
                .read_uint_cells(base + ac..base + ac + sc)
                .ok_or(Errno::EINVAL)? as PAddrr;
            out.push(addr..addr + size);
        }
        Ok(out.into_boxed_slice())
    }

    /// Parse this node's interrupt connections from `interrupts-extended` or
    /// `interrupts` + `interrupt-parent`.
    pub fn interrupts(&self) -> EResult<Vec<IrqEntry>> {
        let mut out = Vec::new();
        if let Some(ext) = self.node.props.get("interrupts-extended") {
            let total = ext.cell_count();
            let mut i = 0;
            while i < total {
                let phandle = ext.read_cell(i).ok_or(Errno::EINVAL)?;
                i += 1;
                let parent = self.dtb.node_by_phandle(phandle).ok_or(Errno::EINVAL)?;
                let icells = cells(parent, "#interrupt-cells")?;
                let entry = read_cells(ext, i, icells)?;
                i += icells;
                out.try_reserve(1)?;
                out.push(IrqEntry {
                    parent,
                    cells: entry,
                });
            }
        } else if let Some(ints) = self.node.props.get("interrupts") {
            let parent = super::irq_parent(self.dtb, self.node).ok_or(Errno::EINVAL)?;
            let icells = cells(parent, "#interrupt-cells")?;
            if icells == 0 {
                return Err(Errno::EINVAL);
            }
            let count = ints.cell_count() / icells;
            for i in 0..count {
                let entry = read_cells(ints, i * icells, icells)?;
                out.try_reserve(1)?;
                out.push(IrqEntry {
                    parent,
                    cells: entry,
                });
            }
        }
        Ok(out)
    }

    /// Look up an already-probed interrupt controller by its node phandle.
    pub fn irqctl_by_phandle(&self, phandle: u32) -> Option<Arc<dyn IrqCtlDevice>> {
        self.phandle_irqctls.get(&phandle).cloned()
    }

    /// Build an [`MmioBus`] for this node: maps `reg`, and resolves its interrupt
    /// parent (a single, already-probed interrupt controller) and source lines.
    /// Returns [`Errno::EAGAIN`] if the interrupt parent has not been probed yet.
    pub fn mmio_bus(&self) -> EResult<Arc<MmioBus>> {
        let reg = self.reg()?;
        let ints = self.interrupts()?;
        let mut irqctl: Option<Arc<dyn IrqCtlDevice>> = None;
        let mut lines = Vec::new();
        lines.try_reserve(ints.len())?;
        for entry in &ints {
            let phandle = entry.parent.phandle.ok_or(Errno::EINVAL)?;
            let ctl = self.irqctl_by_phandle(phandle).ok_or(Errno::EAGAIN)?;
            match &irqctl {
                Some(prev) if !Arc::ptr_eq(prev, &ctl) => return Err(Errno::ENOTSUP),
                Some(_) => {}
                None => irqctl = Some(ctl),
            }
            lines.push(*entry.cells.first().ok_or(Errno::EINVAL)? as u128);
        }
        Ok(Arc::try_new(MmioBus::new(
            Some(self.node),
            reg,
            irqctl,
            lines.into_boxed_slice(),
        ))?)
    }
}

/// Read a `#address-cells`-style count property as a small `usize`.
fn cells(node: &DtbNode, name: &str) -> EResult<usize> {
    node.props
        .get(name)
        .and_then(|p| p.read_cell(0))
        .map(|x| x as usize)
        .ok_or(Errno::EINVAL)
}

/// Read `count` cells starting at cell index `start` from a prop into a boxed slice.
fn read_cells(prop: &crate::device::dtb::DtbProp, start: usize, count: usize) -> EResult<Box<[u32]>> {
    let mut v = Vec::new();
    v.try_reserve(count)?;
    for c in 0..count {
        v.push(prop.read_cell(start + c).ok_or(Errno::EINVAL)?);
    }
    Ok(v.into_boxed_slice())
}

/// Recursively collect all descendant nodes of `root` (depth-first, parents before children).
fn collect_nodes(root: &'static DtbNode, out: &mut Vec<&'static DtbNode>) {
    for child in root.nodes.values() {
        out.push(child);
        collect_nodes(child, out);
    }
}

/// Walk the device tree under `root`, binding registered drivers to matching nodes.
/// Uses deferred probing: a driver returning [`Errno::EAGAIN`] is retried on a later
/// pass, until a full pass makes no progress.
pub fn probe_all(dtb: &'static Dtb, root: &'static DtbNode) {
    let drivers = registered_drivers();
    let mut phandle_irqctls: BTreeMap<u32, Arc<dyn IrqCtlDevice>> = BTreeMap::new();

    let mut worklist = Vec::new();
    collect_nodes(root, &mut worklist);

    loop {
        let mut progressed = false;
        let mut deferred = Vec::new();
        for node in worklist {
            let Some(driver) = drivers.iter().find(|d| d.matches(node)) else {
                continue;
            };
            let ctx = ProbeContext {
                dtb,
                node,
                phandle_irqctls: &phandle_irqctls,
            };
            match driver.probe(&ctx) {
                Ok(device) => {
                    progressed = true;
                    if let Some(phandle) = node.phandle
                        && let Some(ic) = device.clone().as_irqctl()
                    {
                        phandle_irqctls.insert(phandle, ic);
                    }
                    if let Err(e) = registry::register_device(device) {
                        logkf!(LogLevel::Error, "Failed to register {}: {}", node, e);
                    }
                    logkf!(LogLevel::Info, "Probed {} ({})", node, driver.name());
                }
                Err(Errno::EAGAIN) => deferred.push(node),
                Err(e) => {
                    progressed = true;
                    logkf!(LogLevel::Error, "Failed to probe {} ({}): {}", node, driver.name(), e);
                }
            }
        }
        if deferred.is_empty() {
            break;
        }
        if !progressed {
            for node in &deferred {
                logkf!(LogLevel::Warning, "Could not probe {} (dependencies unmet)", node);
            }
            break;
        }
        worklist = deferred;
    }
}
