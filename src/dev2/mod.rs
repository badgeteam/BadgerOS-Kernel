// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::kernel::smp;

#[cfg(feature = "dtb")]
use crate::{
    bindings::log::LogLevel,
    device::dtb::{Dtb, DtbNode, FdtHeader},
};

#[cfg(feature = "dtb")]
use alloc::boxed::Box;

pub mod bus;
pub mod device;
#[cfg(feature = "dtb")]
pub mod driver;
pub mod registry;

/// Get the closest interrup parent for a node.
#[cfg(feature = "dtb")]
pub(crate) fn irq_parent<'a>(dtb: &'a Dtb, node: &DtbNode) -> Option<&'a DtbNode> {
    let irq_parent = node.props.get("interrupt-parent")?;
    if irq_parent.blob.len() != 4 {
        logkf!(LogLevel::Error, "{}: interrupt-parent malformed", node);
        return None;
    }
    let phandle = irq_parent.read_cell(0).unwrap();
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

/// Initialize the device subsystem on DTB systems.
#[cfg(feature = "dtb")]
pub unsafe fn init_dtb(fdt: *const FdtHeader) {
    // Leak the parsed device tree so its nodes are `'static` for the lifetime of the kernel.
    let dtb: &'static Dtb = Box::leak(Box::new(unsafe { Dtb::parse(fdt) }));

    let soc = dtb.root().nodes.get("soc").expect("Missing DTB /soc");
    let cpus = dtb.root().nodes.get("cpus").expect("Missing DTB /cpus");

    // Discover CPUs; this sets up the per-hart CpuLocal state used for interrupt routing.
    smp::init_dtb2(cpus);

    // Probe all devices under /soc. Interrupt controllers (PLICs) probe first and register
    // themselves with the arch root; leaf devices then bind to them via deferred probing.
    // The per-hart RISC-V INTC (cpu-intc) is the arch root and is not itself a device.
    driver::probe_all(dtb, soc);
}
