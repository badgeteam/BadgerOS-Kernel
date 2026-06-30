// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::vec::Vec;
use dtb::{Dtb, DtbNode, DtbProp, spec::FdtHeader};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    kernel,
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
pub fn parse_interrupts(node: &'static DtbNode) -> EResult<Vec<DtbInterrupt>> {
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
    } else {
        logkf!(
            LogLevel::Error,
            "{}: missing interrupts or interrupts-extended",
            node
        );
        return Err(Errno::EINVAL);
    }

    Ok(res)
}

/// Initialize the device subsystem on DTB systems.
pub unsafe fn init(fdt: *const FdtHeader) {
    unsafe {
        assert!((*&raw const DTB).is_none());
        DTB = Some(Dtb::parse(fdt));
    }
    let dtb = get();

    let soc = dtb.root().nodes.get("soc").expect("Missing DTB /soc");
    let cpus = dtb.root().nodes.get("cpus").expect("Missing DTB /cpus");

    // Discover CPUs; this sets up the per-hart CpuLocal state used for interrupt routing.
    kernel::smp::init_dtb2(cpus);

    // Devices under /proc are probed first, any nested devices are to be recursively probed by appropriate drivers.
    unsafe {
        probe(soc);
    }
}

/// Probe for devices by iterating direct child nodes of `parent`.
pub unsafe fn probe(parent: &'static DtbNode) {}
