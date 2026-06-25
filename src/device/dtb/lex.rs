// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::CStr;

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
pub const FDT_END: u32 = 9;

/// FDT token.
#[derive(Clone, Copy)]
pub enum Token<'a> {
    BeginNode(&'a str),
    EndNode,
    Prop(&'a str, &'a [u8]),
}

pub struct TokenStream<'a> {
    pub struct_block: &'a [u32],
    pub string_block: &'a [u8],
}

impl<'a> TokenStream<'a> {
    pub fn next(&mut self) -> Option<Token<'a>> {
        loop {
            assert!(self.struct_block.len() > 0, "Missing FDT_END token");
            let raw = u32::from_be(self.struct_block[0]);
            if raw == FDT_END {
                return None;
            }
            self.struct_block = &self.struct_block[1..];

            match raw {
                FDT_BEGIN_NODE => {
                    let as_bytes: &[u8] = bytemuck::cast_slice(self.struct_block);
                    let name = CStr::from_bytes_until_nul(as_bytes).expect("Unterminated string");
                    self.struct_block = &self.struct_block[(name.count_bytes() + 1).div_ceil(4)..];

                    return Some(Token::BeginNode(
                        name.to_str().expect("Invalid UTF-8 in FDT"),
                    ));
                }
                FDT_END_NODE => return Some(Token::EndNode),
                FDT_PROP => {
                    assert!(
                        self.struct_block.len() >= 2,
                        "Not enough data for FDT property header"
                    );
                    let data_len = u32::from_be(self.struct_block[0]) as usize;
                    let str_off = u32::from_be(self.struct_block[1]) as usize;
                    self.struct_block = &self.struct_block[2..];

                    let data_words = data_len.div_ceil(4);
                    assert!(
                        self.struct_block.len() >= data_words,
                        "Not enough data for FDT property value"
                    );
                    let data: &[u8] = &bytemuck::cast_slice(self.struct_block)[..data_len];
                    self.struct_block = &self.struct_block[data_words..];

                    let name = CStr::from_bytes_until_nul(&self.string_block[str_off..])
                        .expect("Unterminated string");
                    assert!(!name.is_empty(), "Empty property name in FDT");

                    return Some(Token::Prop(
                        name.to_str().expect("Invalid UTF-8 in FDT"),
                        data,
                    ));
                }
                FDT_NOP => (),
                x => panic!("Invalid FDT token 0x{:x}", x),
            }
        }
    }
}
