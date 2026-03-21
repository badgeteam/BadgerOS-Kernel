// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

/// Device addresses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DevAddr {
    /// PCI or PCIe function.
    Pci(PciAddr),
    /// Memory-mapped I/O region.
    Mmio(MmioAddr)
}

/// PCI or PCIe address.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PciAddr {
    /// Distinguishes PCI namespaces (usually 0).
    pub seg: u8,
    /// PCI bus number.
    pub bus: u8,
    /// PCI device number (5-bit).
    pub dev: u8,
    /// PCI function number (3-bit).
    pub func: u8,
}

/// Memory-mapped I/O address.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MmioAddr {
    pub base: usize,
    pub size: usize,
}
