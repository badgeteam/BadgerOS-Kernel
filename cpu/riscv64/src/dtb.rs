// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{bindings::device::dtb::DtbNode, cpu::CpuFeatures, mem::vmm::physmap::PAGING_LEVELS};

#[derive(Debug, Default, Clone, Copy)]
struct IsaSpec {
    pub rv64: bool,
    pub e: bool,
    pub i: bool,
    pub m: bool,
    pub a: bool,
    pub f: bool,
    pub d: bool,
    pub v: bool,
    pub c: bool,
}

/// Parse an ISA string.
fn parse_isa_str(mut isa: &[u8]) -> Option<IsaSpec> {
    let mut spec = IsaSpec::default();
    if isa.starts_with(b"rv32") {
        spec.rv64 = false;
    } else if isa.starts_with(b"rv64") {
        spec.rv64 = true;
    } else {
        return None;
    }
    isa = &isa[4..];

    while isa.len() > 0 {
        match isa[0] {
            b'_' => break,
            b'g' => {
                spec.i = true;
                spec.m = true;
                spec.a = true;
                spec.f = true;
                spec.d = true;
            }
            b'e' => spec.e = true,
            b'i' => spec.i = true,
            b'm' => spec.m = true,
            b'a' => spec.a = true,
            b'f' => spec.f = true,
            b'd' => spec.d = true,
            b'v' => spec.v = true,
            b'c' => spec.c = true,
            _ => (),
        }
        isa = &isa[1..];
    }

    Some(spec)
}

/// Determine whether a CPU is usable by its DTB node.
pub fn is_usable(cpu: &DtbNode) -> Option<CpuFeatures> {
    let isa_prop = cpu.get_prop("riscv,isa")?;
    let mmu_prop = cpu.get_prop("mmu-type")?;
    let isa = isa_prop.bytes();
    let mmu = mmu_prop.bytes();

    let supported_levels = if mmu.eq(b"riscv,sv39\0") {
        3
    } else if mmu.eq(b"riscv,sv48\0") {
        4
    } else if mmu.eq(b"riscv,sv57\0") {
        5
    } else {
        return None;
    };
    if supported_levels < unsafe { PAGING_LEVELS } {
        return None;
    }

    let spec = parse_isa_str(isa)?;
    if !spec.i {
        return None;
    }
    #[cfg(target_arch = "riscv64")]
    if !spec.rv64 {
        return None;
    }
    #[cfg(target_arch = "riscv32")]
    if spec.rv64 {
        return None;
    }
    #[cfg(target_feature = "m")]
    if !spec.m {
        return None;
    }
    #[cfg(target_feature = "c")]
    if !spec.c {
        return None;
    }

    Some(CpuFeatures {
        f32: spec.f,
        f64: spec.d,
        vec: spec.v,
    })
}
