// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
use dtb::{Dtb, DtbNode, spec::FdtHeader};

use crate::{bindings::log::LogLevel, kernel};

/// Get the closest interrup parent for a node.
pub(crate) fn irq_parent<'a>(dtb: &'a Dtb, node: &DtbNode) -> Option<&'a DtbNode> {
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

/// Initialize the device subsystem on DTB systems.
pub unsafe fn init_dtb(fdt: *const FdtHeader) {
    // Leak the parsed device tree so its nodes are `'static` for the lifetime of the kernel.
    let dtb: &'static Dtb = Box::leak(Box::new(unsafe { Dtb::parse(fdt) }));

    let soc = dtb.root().nodes.get("soc").expect("Missing DTB /soc");
    let cpus = dtb.root().nodes.get("cpus").expect("Missing DTB /cpus");

    // Discover CPUs; this sets up the per-hart CpuLocal state used for interrupt routing.
    kernel::smp::init_dtb2(cpus);

    // Probe all devices under /soc. Interrupt controllers (PLICs) probe first and register
    // themselves with the arch root; leaf devices then bind to them via deferred probing.
    // The per-hart RISC-V INTC (cpu-intc) is the arch root and is not itself a device.
    super::driver::probe_dtb(dtb, soc);
}
