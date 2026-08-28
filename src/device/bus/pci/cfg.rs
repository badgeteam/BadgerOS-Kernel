// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::marker::PhantomData;

use num::PrimInt;

#[derive(Clone, Copy)]
pub struct PciRegInfo<T: PciReg> {
    pub offset: u16,
    marker: PhantomData<T>,
}

pub trait PciReg
where
    Self: From<Self::Prim>,
    Self::Prim: From<Self>,
    Self::Prim: PrimInt,
{
    type Prim;
}

impl<T> PciReg for T
where
    T: PrimInt,
{
    type Prim = Self;
}

macro_rules! bit_type {
    (1) => {
        bool
    };
    (2) => {
        u8
    };
    (3) => {
        u8
    };
    (4) => {
        u8
    };
    (5) => {
        u8
    };
    (6) => {
        u8
    };
    (7) => {
        u8
    };
    (8) => {
        u8
    };
}

macro_rules! bit_cast {
    ($val:expr, 1) => {
        $val > 0
    };
    ($val:expr, $x:tt) => {
        $val as _
    };
}

macro_rules! cfg_bitfields {
    ($(
        $(#[$attr:meta])*
        struct $name:ident : $prim:ty {
        $(
            $(#[$fattr:meta])*
            $field:ident : $bits:tt @ $off:tt ;
        )*
    })*) => {$(
        $(#[$attr])*
        pub struct $name(pub $prim);

        impl PciReg for $name {
            type Prim = $prim;
        }

        impl From<$prim> for $name {
            fn from(value: $prim) -> Self {
                Self(value)
            }
        }

        impl From<$name> for $prim {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl $name {
            pub const fn new(
                $(
                    $field: bit_type!($bits)
                ),*
            ) -> Self {
                Self($(
                    (($field as $prim) << $off)
                )|*)
            }

            $(
            $(#[$fattr])*
            pub const fn $field(&self) -> bit_type!($bits) {
                bit_cast!((self.0 >> $off) & (1 as $prim).wrapping_shl($bits).wrapping_sub(1), $bits)
            }
            )*
        }
    )*};
}

macro_rules! cfg_regs {
    ($(
        $(#[$attr:meta])*
        mod $name:ident {
        $(
            $(#[$fattr:meta])*
            const $field:ident : $type:tt @ $off:tt ;
        )*
    })*) => {$(
        $(#[$attr])*
        pub mod $name {
            use super::*;
            $(
            $(#[$fattr])*
            pub const $field: PciRegInfo<$type> = PciRegInfo { offset: $off, marker: PhantomData };
            )*
        }
    )*};
}

cfg_bitfields! {
    /// PCI command register.
    struct CmdReg: u16 {
        /// Bus master / DMA enable.
        dma_enable:     1 @ 2;
        /// Parity error response.
        parity_resp:    1 @ 6;
        /// SERR# non-fatal error reporting enable.
        serr_enable:    1 @ 8;
        /// Disable and deassert INTx.
        irq_disable:    1 @ 10;
    }

    /// PCI status register.
    struct StatReg: u16 {
        /// Internal interrupt status for INTx emulation.
        intx_status:        1 @ 3;
        /// Has extended capabilities list.
        extcap:             1 @ 4;
        /// Master data parity error detected; write 1 to clear.
        data_parity_err:    1 @ 8;
        /// Singalled target abort.
        sig_target_abort:   1 @ 11;
        /// Received target abort.
        recv_target_abort:  1 @ 12;
        /// Received master abort.
        recv_master_abort:  1 @ 13;
        /// Signalled system error.
        sig_sys_err:        1 @ 14;
        /// Detected parity error.
        parity_err:         1 @ 15;
    }
}

cfg_regs! {
    /// Common configuration space registers.
    mod common {
        /// Vendor ID.
        const VENDOR:       u16     @ 0x00;
        /// Device ID.
        const DEVICE:       u16     @ 0x02;

        /// Command register.
        const COMMAND:      CmdReg  @ 0x04;
        /// Status register.
        const STATUS:       StatReg @ 0x06;

        /// Revision ID.
        const REVISION:     u8      @ 0x08;
        /// Programming interface.
        const PROGIF:       u8      @ 0x09;
        /// Subclass.
        const SUBCLASS:     u8      @ 0x0a;
        /// Base class.
        const BASECLASS:    u8      @ 0x0b;

        /// Header type.
        const HDR_TYPE:     u8      @ 0x0e;

        /// Capabilities list pointer.
        const CAPS_PTR:     u8      @ 0x34;
        /// Interrupt line register (for software use).
        const IRQ_LINE:     u8      @ 0x3c;
        /// Interrupt pin register.
        const IRQ_PIN:      u8      @ 0x3d;
    }

    /// Type 0 (PCI device function) configuration space registers.
    mod device {
        /// Base address register 0.
        const BAR0:         u32     @ 0x10;
        /// Base address register 1.
        const BAR1:         u32     @ 0x14;
        /// Base address register 2.
        const BAR2:         u32     @ 0x18;
        /// Base address register 3.
        const BAR3:         u32     @ 0x1c;
        /// Base address register 4.
        const BAR4:         u32     @ 0x20;
        /// Base address register 5.
        const BAR5:         u32     @ 0x24;
    }
}

/// BAR flag: Is I/O BAR.
pub const BAR_FLAG_IO: u32 = 1;
/// BAR flag: Is a 64-bit memory BAR.
pub const BAR_FLAG_64BIT: u32 = 4;
/// I/O BAR address mask.
pub const BAR_IO_ADDR_MASK: u32 = 0xfffffffc;
/// 32-bit memory BAR address mask.
pub const BAR_MEM32_ADDR_MASK: u32 = 0xfffffff0;
/// 64-bit memory BAR address mask.
pub const BAR_MEM64_ADDR_MASK: u64 = 0xfffffffffffffff0;
/// Memory BAR flag: Is prefetchable.
pub const BAR_FLAG_PREFETCH: u32 = 8;
