// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ops::Range;

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use dtb::{Dtb, DtbNode, DtbProp, spec::FdtHeader};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    cpu::PhysCpuID,
    dev2::{bus::soc::SocIrqParent, probe, registry},
    kernel::{self, smp},
    mem::pmm::PAddrr,
};

use super::bus::{
    Bus,
    soc::{SocBus, SocIrqExt},
};

/// The device tree.
static mut DTB: Option<Dtb> = None;

/// Get the device tree.
pub fn get() -> &'static Dtb {
    unsafe { (*&raw const DTB).as_ref().expect("DTB is uninitialized") }
}

/// Get the closest interrupt parent for a node.
pub fn irq_parent<'a>(node: &'a DtbNode) -> Option<&'a DtbNode> {
    let dtb = get();
    let irq_parent = node.prop("interrupt-parent")?;
    let Some(phandle) = irq_parent.read_u32() else {
        logkf!(LogLevel::Error, "{}: interrupt-parent malformed", node);
        return None;
    };
    let res = dtb.node_by_phandle(phandle);
    if res.is_none() {
        logkf!(
            LogLevel::Error,
            "{}: interrupt-parent {} not found",
            node,
            phandle
        );
    }
    res
}

/// Outgoing DTB interrupt connection.
pub struct DtbInterrupt {
    pub parent: &'static DtbNode,
    pub vector: u128,
}

/// Parses an interrupt specifier given an interrupt parent.
fn parse_irq_spec(
    node: &'static DtbNode,
    parent: &'static DtbNode,
    prop: &'static DtbProp,
    index: &mut usize,
) -> EResult<DtbInterrupt> {
    let Some(irq_cells) = parent.irq_cells else {
        logkf!(LogLevel::Error, "{}: missing #interrupt-cells", parent);
        return Err(Errno::EINVAL);
    };

    match prop.read_uint_cells(*index..*index + irq_cells as usize) {
        Some(vector) => {
            *index += irq_cells as usize;
            Ok(DtbInterrupt { parent, vector })
        }
        None => {
            logkf!(
                LogLevel::Error,
                "{}: not enough cells for {}",
                node,
                &prop.name
            );
            Err(Errno::EINVAL)
        }
    }
}

/// Parse the `interrupts` property of a DTB node.
pub fn parse_interrupts(node: &'static DtbNode) -> EResult<Box<[DtbInterrupt]>> {
    let mut res = Vec::new();
    if let Some(irqext_prop) = node.prop("interrupts-extended") {
        let Some(n_cell) = irqext_prop.cell_count() else {
            logkf!(LogLevel::Error, "{}: malformed interrups-extended", node);
            return Err(Errno::EINVAL);
        };

        let mut index = 0;
        while index < n_cell {
            // Format: parent phandle, interrupt specifier.
            let parent_phandle = irqext_prop.read_cell(index).unwrap();
            let Some(parent) = get().node_by_phandle(parent_phandle) else {
                logkf!(LogLevel::Error, "phandle {} not found", parent_phandle);
                return Err(Errno::EINVAL);
            };
            index += 1;

            let irq = parse_irq_spec(node, parent, irqext_prop, &mut index)?;
            res.try_reserve(1)?;
            res.push(irq);
        }
    } else if let Some(irq_prop) = node.prop("interrupts") {
        let Some(n_cell) = irq_prop.cell_count() else {
            logkf!(LogLevel::Error, "{}: malformed interrups", node);
            return Err(Errno::EINVAL);
        };
        let Some(parent) = irq_parent(node) else {
            logkf!(LogLevel::Error, "{}: missing interrupt-parent", node);
            return Err(Errno::EINVAL);
        };

        let mut index = 0;
        while index < n_cell {
            // Format: list of interrupt specifiers.
            let irq = parse_irq_spec(node, parent, irq_prop, &mut index)?;
            res.try_reserve(1)?;
            res.push(irq);
        }
    }

    Ok(res.into_boxed_slice())
}

/// Parse the `reg` property of a DTB node.
pub fn parse_reg(node: &DtbNode) -> EResult<Box<[Range<u128>]>> {
    let parent = node.parent().ok_or(Errno::EINVAL)?;
    let Some(addr_cells) = parent.addr_cells else {
        logkf!(LogLevel::Error, "{}: missing #address-cells", parent);
        return Err(Errno::EINVAL);
    };
    let addr_cells = addr_cells as usize;
    let size_cells = parent.size_cells.unwrap_or(0) as usize;
    let entry_cells = addr_cells + size_cells;

    let Some(prop) = node.prop("reg") else {
        logkf!(LogLevel::Error, "{}: Missing reg", node);
        return Err(Errno::EINVAL);
    };
    let cell_count = prop.blob.len() / 4;
    if cell_count % entry_cells != 0 || prop.blob.len() % 4 != 0 {
        logkf!(LogLevel::Error, "{}: Malformed reg", node);
        return Err(Errno::EINVAL);
    };

    let mut res = Vec::new();
    res.try_reserve_exact(cell_count / entry_cells)?;
    for i in 0..cell_count / entry_cells {
        let addr = prop
            .read_uint_cells(i * entry_cells..i * entry_cells + addr_cells)
            .unwrap();
        let size = prop
            .read_uint_cells(
                i * entry_cells + addr_cells..i * entry_cells + addr_cells + size_cells,
            )
            .unwrap();
        res.push(addr..addr + size);
    }

    Ok(res.into_boxed_slice())
}

/// Initialize the device subsystem on DTB systems.
pub unsafe fn init(fdt: *const FdtHeader) {
    unsafe {
        assert!((*&raw const DTB).is_none());
        DTB = Some(Dtb::parse(fdt));
    }
    let dtb = get();

    #[cfg(debug_assertions)]
    {
        logkf!(LogLevel::Debug, "DTB:");
        printf!("{}", dtb);
    }

    let soc = dtb.root().nodes.get("soc").expect("Missing DTB /soc");
    let cpus = dtb.root().nodes.get("cpus").expect("Missing DTB /cpus");

    // Discover CPUs; this sets up the per-hart CpuLocal state used for interrupt routing.
    kernel::smp::init_dtb2(cpus);

    // Devices under /proc are probed first, any nested devices are to be recursively probed by appropriate drivers.
    unsafe {
        probe(soc, &(probe_soc_factory as _));
    }
}

/// Data parsed from the DTB for device nodes.
pub struct DeviceNode {
    /// Associated DTB node.
    pub node: &'static DtbNode,
    /// Value of the `reg` property.
    pub reg: Box<[Range<u128>]>,
    /// Decoded value of the `interrupts` or `interrupts-extended` property.
    pub irq: Box<[DtbInterrupt]>,
}

/// Bus factory for [`probe()`] the generates [`SocBus`] instances.
pub unsafe fn probe_soc_factory(node: DeviceNode) -> EResult<Arc<dyn Bus>> {
    fn get_parent(node: &'static DtbNode) -> EResult<Option<SocIrqParent>> {
        let cpus = get().node("cpus").unwrap();

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
        } else if let Some(irq_bus) = registry::bus_by_node(node) {
            // This node is a DTB device.
            if let Some(device) = irq_bus.owner() {
                if let Some(irqctl) = device.as_irqctl() {
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

    let bus = Arc::try_new(SocBus::new(
        Some(node.node),
        node.reg
            .into_iter()
            .map(|x| x.start as PAddrr..x.end as PAddrr)
            .collect(),
        irq_ext.into_boxed_slice(),
    ))?;

    Ok(bus)
}

/// Probe for devices by iterating direct child nodes of `parent`.
pub unsafe fn probe(
    parent: &'static DtbNode,
    bus_factory: &unsafe fn(DeviceNode) -> EResult<Arc<dyn Bus>>,
) {
    let mut work: Vec<&'static DtbNode> = parent.nodes.values().map(AsRef::as_ref).collect();
    let mut progress = true;

    while progress {
        progress = false;

        let mut i = 0;
        while let Some(&node) = work.get(i) {
            let Some(compatible) = node.prop("compatible") else {
                work.remove(i);
                continue;
            };

            let res = try {
                let reg = parse_reg(node)?;
                let irq = parse_interrupts(node)?;
                unsafe {
                    let bus = bus_factory(DeviceNode { node, reg, irq })?;
                    registry::register_bus(bus.clone())?;
                    logkf!(
                        LogLevel::Info,
                        "Added DTB node {} as bus {}",
                        node,
                        bus.id()
                    );
                    for str in compatible.strings() {
                        logkf!(LogLevel::Info, "  -> \"{}\"", str);
                    }
                    if let Some(driver) = probe::probe_bus(bus) {
                        logkf!(LogLevel::Info, "  Probed driver \"{}\"", driver.name());
                    }
                }
            };

            match res {
                Ok(()) => {
                    progress = true;
                    work.remove(i);
                }
                Err(Errno::EAGAIN) => {
                    // Node will be retried next iteration.
                    i += 1;
                }
                Err(x) => {
                    logkf!(LogLevel::Error, "{}: Error parsing DTB node: {}", node, x);
                    work.remove(i);
                }
            }
        }
    }

    if !work.is_empty() {
        logkf!(
            LogLevel::Warning,
            "Unable to build {} DTB node{}:",
            work.len(),
            if work.len() == 1 { "" } else { "s" }
        );
        for orphan in work {
            logkf!(LogLevel::Warning, "  -> {}", orphan);
        }
    }
}
