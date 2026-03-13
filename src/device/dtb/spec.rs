// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

//! Defines structs for the FDT layout in memory; they are all big-endian.

/// Calls conversion function `$func` on integers in `$type`.
#[rustfmt::skip]
macro_rules! int_conv {
    // Implementations on all primitive integer types.
    ($func: ident,  $name: expr,  u8  ) => { $name = u8  ::$func($name); };
    ($func: ident,  $name: expr,  i8  ) => { $name = i8  ::$func($name); };
    ($func: ident,  $name: expr,  u16 ) => { $name = u16 ::$func($name); };
    ($func: ident,  $name: expr,  i16 ) => { $name = i16 ::$func($name); };
    ($func: ident,  $name: expr,  u32 ) => { $name = u32 ::$func($name); };
    ($func: ident,  $name: expr,  i32 ) => { $name = i32 ::$func($name); };
    ($func: ident,  $name: expr,  u64 ) => { $name = u64 ::$func($name); };
    ($func: ident,  $name: expr,  i64 ) => { $name = i64 ::$func($name); };
    ($func: ident,  $name: expr,  u128) => { $name = u128::$func($name); };
    ($func: ident,  $name: expr,  i128) => { $name = i128::$func($name); };
    // Implementation on arrays.
    ($func: ident,  $name: expr,  [$type: tt; $count: expr]) => {
        for __i in 0..$count {
            int_conv!{ $func, $name[__i], $type }
        }
    };
}

/// Helper macro for defining an FDT struct.
macro_rules! fdt_struct {
    ($(#[doc = $structdoc: expr])*
    struct $structname: ident {
        $(
            $(#[doc = $fielddoc: expr])*
            $name: ident : $type: tt
        ),*
        $(,)?
    }) => {
        // Define base struct.
        #[repr(C)]
        #[derive(Clone, Copy)]
        $(#[doc = $structdoc])*
        pub struct $structname {
            $(
                $(#[doc = $fielddoc])*
                $name: $type,
            )*
        }
        // Define conversion to big-endian.
        impl $structname {
            /// Convert all integers into big-endian; byte-swap on little-endian machines.
            pub fn from_be(mut self) -> Self {
                $(int_conv!(from_be, self.$name, $type);)*
                self
            }
            /// Convert all integers from big-endian; byte-swap on little-endian machines.
            pub fn to_be(self) -> Self {
                self.from_be()
            }
        }
    };
}

/// Flattened Device Tree header.
#[derive(Clone, Copy)]
pub struct FdtHeader {
    pub magic: u32,
    pub totalsize: u32,
    pub struct_offset: u32,
    pub string_offset: u32,
    pub memresv_offset: u32,
    pub version: u32,
    pub compat_version: u32,
    pub bsp_cpuid: u32,
    pub string_size: u32,
    pub struct_size: u32,
}

impl FdtHeader {
    /// FDT header magic value.
    pub const MAGIC: u32 = 0xd00dfeed;

    /// Convert all integers into big-endian; byte-swap on little-endian machines.
    pub fn from_be(mut self) -> Self {
        self.magic = u32::from_be(self.magic);
        self.totalsize = u32::from_be(self.totalsize);
        self.struct_offset = u32::from_be(self.struct_offset);
        self.string_offset = u32::from_be(self.string_offset);
        self.memresv_offset = u32::from_be(self.memresv_offset);
        self.version = u32::from_be(self.version);
        self.compat_version = u32::from_be(self.compat_version);
        self.bsp_cpuid = u32::from_be(self.bsp_cpuid);
        self.string_size = u32::from_be(self.string_size);
        self.struct_size = u32::from_be(self.struct_size);
        self
    }

    /// Convert all integers from big-endian; byte-swap on little-endian machines.
    pub fn to_be(self) -> Self {
        self.from_be()
    }
}

pub const FDT_BEGIN_NODE: u32 = 1;
pub const FDT_END_NODE: u32 = 2;
pub const FDT_PROP: u32 = 3;
pub const FDT_NOP: u32 = 4;
pub const FDT_END: u32 = 5;
