// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::fmt::Display;

/// PCI interrupt number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PciIrq {
    IntA = 1,
    IntB = 2,
    IntC = 3,
    IntD = 4,
}

impl TryFrom<u128> for PciIrq {
    type Error = ();

    #[inline(always)]
    fn try_from(value: u128) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::IntA),
            2 => Ok(Self::IntB),
            3 => Ok(Self::IntC),
            4 => Ok(Self::IntD),
            _ => Err(()),
        }
    }
}

impl TryFrom<u32> for PciIrq {
    type Error = ();

    #[inline(always)]
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::try_from(value as u128)
    }
}

impl TryFrom<u8> for PciIrq {
    type Error = ();

    #[inline(always)]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::try_from(value as u128)
    }
}

/// PCI address space code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PciSeg {
    /// PCI configuration space.
    Config = 0,
    /// BAR as I/O space.
    IO = 1,
    /// BAR as 32-bit memory.
    Mem32 = 2,
    /// BAR as 64-bit memory.
    Mem64 = 3,
}

/// PCI physical address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciPAddr {
    /// Low 32 bits of address.
    pub low: u32,
    /// High 32 bits of address.
    pub high: u32,
    /// Attributes, BDF, etc.
    pub attr: u32,
}

impl PciPAddr {
    pub const fn new(
        non_reloc: bool,
        prefetch: bool,
        aliased: bool,
        seg: PciSeg,
        dev_addr: PciAddr,
        regno: u8,
        seg_addr: u64,
    ) -> Self {
        Self {
            low: seg_addr as u32,
            high: (seg_addr >> 32) as u32,
            attr: regno as u32
                | (dev_addr.0 as u32) << 8
                | (seg as u32) << 16
                | (aliased as u32) << 29
                | (prefetch as u32) << 30
                | (non_reloc as u32) << 31,
        }
    }

    pub const fn new_config(dev_addr: PciAddr, regno: u8) -> Self {
        Self::new(false, false, false, PciSeg::Config, dev_addr, regno, 0)
    }

    pub const fn seg_addr(&self) -> u64 {
        (self.high as u64) << 32 | self.low as u64
    }
    pub const fn regno(&self) -> u8 {
        self.attr as u8
    }
    pub const fn dev_addr(&self) -> PciAddr {
        PciAddr((self.attr >> 8) as u16)
    }
    pub const fn seg(&self) -> PciSeg {
        match (self.attr >> 16) & 3 {
            0 => PciSeg::Config,
            1 => PciSeg::IO,
            2 => PciSeg::Mem32,
            3 => PciSeg::Mem64,
            _ => unreachable!(),
        }
    }
    pub const fn non_reloc(&self) -> bool {
        self.attr & (1 << 31) != 0
    }
    pub const fn prefetch(&self) -> bool {
        self.attr & (1 << 30) != 0
    }
    pub const fn aliased(&self) -> bool {
        self.attr & (1 << 29) != 0
    }
}

impl From<PciPAddr> for u128 {
    fn from(value: PciPAddr) -> Self {
        (value.attr as u128) << 64 | (value.high as u128) << 32 | value.low as u128
    }
}

impl From<u128> for PciPAddr {
    fn from(value: u128) -> Self {
        Self {
            attr: (value >> 64) as u32,
            high: (value >> 32) as u32,
            low: value as u32,
        }
    }
}

/// PCI function address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PciAddr(pub u16);

impl PciAddr {
    pub const fn new(bus: u8, dev: u8, func: u8) -> Self {
        Self((bus as u16) << 8 | (dev as u16) << 3 | func as u16)
    }

    pub const fn bus(&self) -> u8 {
        (self.0 >> 8) as u8
    }
    pub const fn dev(&self) -> u8 {
        (self.0 >> 3) as u8 & 0x1f
    }
    pub const fn func(&self) -> u8 {
        self.0 as u8 & 7
    }
}

impl Display for PciAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "{:02x}:{:02x}.{}",
            self.bus(),
            self.dev(),
            self.func()
        ))
    }
}
