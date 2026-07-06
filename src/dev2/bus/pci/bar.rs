// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::mem::pmm::PAddrr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarType {
    IO,
    Mem32,
    Mem64,
}

/// Information about a device's Base Address Register.
#[derive(Debug, Clone, Copy)]
pub struct BarInfo {
    /// Memory space address.
    pub seg_addr: u64,
    /// Translated CPU physical address.
    pub cpu_paddr: PAddrr,
    /// BAR size in bytes.
    pub size: usize,
    /// BAR type.
    pub type_: BarType,
    /// Memory BARs: is prefetchable.
    /// Always false for I/O BARs.
    pub prefetch: bool,
}
