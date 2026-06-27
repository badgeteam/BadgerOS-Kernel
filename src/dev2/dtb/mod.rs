// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use dtb::{Dtb, DtbNode, spec::FdtHeader};

use crate::{bindings::log::LogLevel, kernel};

/// The device tree.
static mut DTB: Option<Dtb> = None;

/// Get the device tree.
pub fn get() -> &'static Dtb {
    unsafe { (*&raw const DTB).as_ref().expect("DTB is uninitialized") }
}

/// Get the closest interrup parent for a node.
pub(crate) fn irq_parent<'a>(node: &DtbNode) -> Option<&'a DtbNode> {
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

/// Initialize the device subsystem on DTB systems.
pub unsafe fn init_dtb(fdt: *const FdtHeader) {
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
    super::driver::probe_dtb(soc);
}
