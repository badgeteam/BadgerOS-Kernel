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
    cpu,
    dev2::{probe, registry},
    kernel::{self},
    misc::kparam,
};

use super::bus::{Bus, soc::SocBus};

/// The device tree.
static mut DTB: Option<Dtb> = None;

/// Get the device tree.
pub fn get() -> &'static Dtb {
    unsafe { (*&raw const DTB).as_ref().expect("DTB is uninitialized") }
}

/// Get the closest interrupt parent for a node.
pub fn irq_parent(node: &'static DtbNode) -> Option<&'static DtbNode> {
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
pub struct DtbIrq {
    pub parent: &'static DtbNode,
    pub vector: u128,
}

/// DTB interrupt map.
pub struct DtbIrqMap {
    pub addr_mask: u128,
    pub vector_mask: u128,
    pub map: Box<[DtbIrqMapEntry]>,
}

/// A single entry in a [`DtbIrqMap`].
pub struct DtbIrqMapEntry {
    pub addr: u128,
    pub irq: u128,
    pub target: DtbIrq,
}

/// An address-mapping range.
pub struct DtbRange {
    pub child_addr: u128,
    pub parent_addr: u128,
    pub size: u128,
}

/// Parses an interrupt specifier given an interrupt parent.
fn parse_irq_spec(
    node: &'static DtbNode,
    parent: &'static DtbNode,
    prop: &'static DtbProp,
    index: &mut usize,
) -> EResult<DtbIrq> {
    let Some(irq_cells) = parent.irq_cells else {
        logkf!(LogLevel::Error, "{}: missing #interrupt-cells", parent);
        return Err(Errno::EINVAL);
    };

    match prop.read_uint_cells(*index, irq_cells as usize) {
        Some(vector) => {
            *index += irq_cells as usize;
            Ok(DtbIrq { parent, vector })
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
pub fn parse_interrupts(node: &'static DtbNode) -> EResult<Box<[DtbIrq]>> {
    let mut res = Vec::new();
    if let Some(irqext_prop) = node.prop("interrupts-extended") {
        let Some(n_cell) = irqext_prop.cell_count() else {
            logkf!(LogLevel::Error, "{}: malformed interrupts-extended", node);
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
            logkf!(LogLevel::Error, "{}: malformed interrupts", node);
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

/// Parse the `interrupt-map` propery of a DTB node.
pub fn parse_interrupt_map(node: &'static DtbNode) -> EResult<DtbIrqMap> {
    let addr_cells = node.addr_cells.ok_or(Errno::EINVAL)? as usize;
    let irq_cells = node.irq_cells.ok_or(Errno::EINVAL)? as usize;

    let Some(map_prop) = node.prop("interrupt-map") else {
        logkf!(LogLevel::Error, "{}: missing interrupt-map", node);
        return Err(Errno::EINVAL);
    };
    let Some(mask_prop) = node.prop("interrupt-map-mask") else {
        logkf!(LogLevel::Error, "{}: missing interrupt-map-mask", node);
        return Err(Errno::EINVAL);
    };
    if mask_prop.cell_count() != Some(addr_cells + irq_cells) {
        logkf!(LogLevel::Error, "{}: malformed interrupt-map-mask", node);
        return Err(Errno::EINVAL);
    }
    let addr_mask = mask_prop.read_uint_cells(0, addr_cells).unwrap();
    let vector_mask = mask_prop.read_uint_cells(addr_cells, irq_cells).unwrap();

    let map_cells = map_prop.cell_count().ok_or(Errno::EINVAL)?;

    let mut map = Vec::new();
    let mut i = 0;
    while i < map_cells {
        if map_cells < i + addr_cells + irq_cells + 1 {
            logkf!(LogLevel::Error, "{}: malformed interrupt-map", node);
            return Err(Errno::EINVAL);
        }

        let addr = map_prop.read_uint_cells(i, addr_cells).unwrap();
        i += addr_cells;
        let irq = map_prop.read_uint_cells(i, irq_cells).unwrap();
        i += irq_cells;

        let phandle = map_prop.read_cell(i).unwrap();
        i += 1;
        let Some(parent) = get().node_by_phandle(phandle) else {
            logkf!(LogLevel::Error, "phandle {} not found", phandle);
            return Err(Errno::EINVAL);
        };
        let parent_irq = parse_irq_spec(node, parent, map_prop, &mut i)?;

        map.try_reserve(1)?;
        map.push(DtbIrqMapEntry {
            addr,
            irq,
            target: parent_irq,
        });
    }

    Ok(DtbIrqMap {
        addr_mask,
        vector_mask,
        map: map.into_boxed_slice(),
    })
}

/// Parse the `reg` property of a DTB node.
pub fn parse_reg(node: &'static DtbNode) -> EResult<Box<[Range<u128>]>> {
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
        let addr = prop.read_uint_cells(i * entry_cells, addr_cells).unwrap();
        let size = prop
            .read_uint_cells(i * entry_cells + addr_cells, size_cells)
            .unwrap();
        res.push(addr..addr + size);
    }

    Ok(res.into_boxed_slice())
}

/// Parse the `ranges` property of a DTB node.
pub fn parse_ranges(node: &'static DtbNode) -> EResult<Box<[DtbRange]>> {
    let Some(ranges_prop) = node.prop("ranges") else {
        logkf!(LogLevel::Error, "{}: missing ranges", node);
        return Err(Errno::EINVAL);
    };
    let Some(parent_addr_cells) = node.parent().ok_or(Errno::EINVAL)?.addr_cells else {
        logkf!(LogLevel::Error, "{}: missing #address-cells", node);
        return Err(Errno::EINVAL);
    };
    let Some(addr_cells) = node.addr_cells else {
        logkf!(LogLevel::Error, "{}: missing #address-cells", node);
        return Err(Errno::EINVAL);
    };
    let parent_addr_cells = parent_addr_cells as usize;
    let addr_cells = addr_cells as usize;
    let size_cells = node.size_cells.unwrap_or(1) as usize;

    let entry_cells = parent_addr_cells + addr_cells + size_cells;
    let entry_count = ranges_prop.blob.len() / (4 * entry_cells);
    if ranges_prop.blob.len() % (4 * entry_cells) != 0 {
        logkf!(LogLevel::Error, "{}: malformed ranges", node);
        return Err(Errno::EINVAL);
    }

    let mut ranges = Vec::try_with_capacity(entry_count)?;
    for i in 0..entry_count {
        ranges.push(DtbRange {
            child_addr: ranges_prop
                .read_uint_cells(i * entry_cells, addr_cells)
                .unwrap(),
            parent_addr: ranges_prop
                .read_uint_cells(i * entry_cells + addr_cells, parent_addr_cells)
                .unwrap(),
            size: ranges_prop
                .read_uint_cells(i * entry_cells + addr_cells + parent_addr_cells, size_cells)
                .unwrap(),
        });
    }

    Ok(ranges.into_boxed_slice())
}

/// Initialize the device subsystem on DTB systems.
pub unsafe fn init(fdt: *const FdtHeader) {
    unsafe {
        assert!((*&raw const DTB).is_none());
        DTB = Some(Dtb::parse(fdt));
    }
    let dtb = get();

    if kparam::get_kparam("DUMPDTB").is_some() {
        logkf!(LogLevel::Debug, "DTB:");
        printf!("{}", dtb);
    }

    let soc = dtb.root().nodes.get("soc").expect("Missing DTB /soc");
    let cpus = dtb.root().nodes.get("cpus").expect("Missing DTB /cpus");

    // Set up the CPU-local timers.
    cpu::timer::init_dtb2(cpus);
    // Discover CPUs; this sets up the per-hart CpuLocal state used for interrupt routing.
    kernel::smp::init_dtb2(cpus);

    // Devices under /proc are probed first, any nested devices are to be recursively probed by appropriate drivers.
    unsafe {
        probe(soc, &(SocBus::factory as _));
    }
}

/// Data parsed from the DTB for device nodes.
pub struct DeviceNode {
    /// Associated DTB node.
    pub node: &'static DtbNode,
    /// Value of the `reg` property.
    pub reg: Box<[Range<u128>]>,
    /// Decoded value of the `interrupts` or `interrupts-extended` property.
    pub irq: Box<[DtbIrq]>,
    /// Decoded value of the `interrupt-map` property.
    pub irq_map: Option<DtbIrqMap>,
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
                let irq_map;
                if let Some(_) = node.prop("interrupt-map") {
                    irq_map = Some(parse_interrupt_map(node)?);
                } else {
                    irq_map = None;
                }
                unsafe {
                    let bus = bus_factory(DeviceNode {
                        node,
                        reg,
                        irq,
                        irq_map,
                    })?;
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
                    probe::probe_bus(bus);
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
