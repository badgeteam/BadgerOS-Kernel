// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ffi::CStr,
    fmt::{Display, Write},
    ops::{Deref, Range},
    ptr::null,
};

use alloc::{boxed::Box, collections::btree_map::BTreeMap, string::String};
use lex::*;

use crate::bindings::log::LogLevel;

mod lex;

pub type FdtHeader = lex::FdtHeader;

/// Loaded device tree structure.
pub struct Dtb {
    /// DTB root node.
    root: Box<DtbNode>,
    /// Map from phandle to node.
    by_phandle: BTreeMap<u32, *const DtbNode>,
}
unsafe impl Send for Dtb {}
unsafe impl Sync for Dtb {}

impl Dtb {
    pub const MIN_SUPPORTED: u32 = 16;
    pub const MAX_SUPPORTED: u32 = 17;

    /// Parse DTB from an FDT pointer.
    /// # Panics
    /// - If the FDT is malformed.
    pub unsafe fn parse(fdt: *const FdtHeader) -> Self {
        let header = FdtHeader::from_be(unsafe { *fdt });
        assert!(header.magic == FdtHeader::MAGIC, "Invalid FDT magic");

        let mut tkn = TokenStream {
            struct_block: unsafe {
                &*core::ptr::slice_from_raw_parts(
                    (fdt as usize + header.struct_offset as usize) as *const u32,
                    header.struct_size as usize,
                )
            },
            string_block: unsafe {
                &*core::ptr::slice_from_raw_parts(
                    (fdt as usize + header.string_offset as usize) as *const u8,
                    header.string_size as usize,
                )
            },
        };

        if let Some(Token::BeginNode(name)) = tkn.next() {
            assert!(name.is_empty(), "FDT root node's name must be empty");
        } else {
            panic!("FDT must begin with FDT_BEGIN_NODE");
        }

        let mut by_phandle = BTreeMap::new();
        let root = unsafe { DtbNode::parse(&mut tkn, &mut by_phandle, null(), "") };
        assert!(tkn.next().is_none(), "Unexpected extra data in FDT");

        Self { root, by_phandle }
    }

    /// DTB root node.
    pub fn root(&self) -> &DtbNode {
        &self.root
    }

    /// Get a node by its phandle.
    pub fn node_by_phandle(&self, phandle: u32) -> Option<&DtbNode> {
        self.by_phandle.get(&phandle).map(|x| unsafe { &**x })
    }
}

/// Device tree node.
#[derive(Debug)]
pub struct DtbNode {
    /// This node's name.
    pub name: String,
    /// Parent node, if any.
    parent: *const DtbNode,
    /// Cached phandle, if any.
    pub phandle: Option<u32>,
    /// Child nodes.
    pub nodes: BTreeMap<String, Box<DtbNode>>,
    /// Child props.
    pub props: BTreeMap<String, DtbProp>,
}
unsafe impl Send for DtbNode {}
unsafe impl Sync for DtbNode {}

impl DtbNode {
    unsafe fn parse(
        tkn: &mut TokenStream<'_>,
        by_phandle: &mut BTreeMap<u32, *const DtbNode>,
        parent: *const DtbNode,
        name: &str,
    ) -> Box<Self> {
        let mut this = Box::new(Self {
            name: name.into(),
            parent,
            phandle: None,
            nodes: BTreeMap::new(),
            props: BTreeMap::new(),
        });

        loop {
            match tkn.next().expect("Unexpected FDT_END") {
                Token::BeginNode(name) => {
                    let child = unsafe { Self::parse(tkn, by_phandle, this.deref(), name) };
                    this.nodes.insert(name.into(), child);
                }
                Token::EndNode => break,
                Token::Prop(name, blob) => {
                    let child = DtbProp {
                        name: name.into(),
                        parent: this.deref(),
                        blob: blob.into(),
                    };
                    this.props.insert(name.into(), child);
                }
            }
        }

        this.phandle = this.props.get("phandle").map(|prop| {
            assert!(prop.cell_count() == Some(1));
            prop.read_cell(0).unwrap()
        });

        if let Some(phandle) = this.phandle {
            by_phandle.insert(phandle, this.deref());
        }

        this
    }

    /// Get the parent node.
    pub fn parent(&self) -> Option<&DtbNode> {
        if self.parent.is_null() {
            return None;
        }
        Some(unsafe { &*self.parent })
    }

    /// Whether this node's `compatible` property contains the given string.
    pub fn is_compatible(&self, want: &str) -> bool {
        self.props
            .get("compatible")
            .is_some_and(|prop| prop.strings().any(|s| s == want))
    }

    /// Whether this node's `compatible` property contains any of the given strings.
    pub fn is_compatible_any(&self, want: &[&str]) -> bool {
        want.iter().any(|w| self.is_compatible(w))
    }
}

impl Display for DtbNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Count how deep in the DTB this node is.
        let mut depth = 0;
        let mut cur = self;
        while let Some(node) = cur.parent() {
            depth += 1;
            cur = node;
        }

        // Iteratively walk down so the path is printed in proper order.
        for x in (1..depth).rev() {
            let mut cur = self;
            for _ in 0..x {
                cur = cur.parent().unwrap();
            }
            f.write_str(&cur.name)?;
            f.write_char('/')?;
        }
        f.write_str(&self.name)?;

        Ok(())
    }
}

/// Device tree property.
#[derive(Debug)]
pub struct DtbProp {
    /// This prop's name.
    pub name: String,
    /// Parent node, if any.
    parent: *const DtbNode,
    /// Binary value.
    pub blob: Box<[u8]>,
}
unsafe impl Send for DtbProp {}
unsafe impl Sync for DtbProp {}

impl DtbProp {
    /// Get the parent node.
    pub fn parent(&self) -> &DtbNode {
        unsafe { &*self.parent }
    }

    /// Number of `<u32>` cells in this prop.
    pub fn cell_count(&self) -> Option<usize> {
        (self.blob.len() % 4 == 0).then_some(self.blob.len() / 4)
    }

    /// Iterate the NUL-separated strings in this prop (e.g. a `compatible` list).
    pub fn strings(&self) -> impl Iterator<Item = &str> {
        self.blob
            .split(|&b| b == 0)
            .filter(|s| !s.is_empty())
            .filter_map(|s| str::from_utf8(s).ok())
    }

    /// Read a cell in this prop.
    pub fn read_cell(&self, cell: usize) -> Option<u32> {
        self.read_uint_cells(cell..cell + 1).map(|x| x as u32)
    }

    /// Read this prop as some integer.
    pub fn read_uint_cells(&self, cells: Range<usize>) -> Option<u128> {
        debug_assert!(cells.len() <= 4);
        if self.blob.len() / 4 < cells.end {
            logkf!(
                LogLevel::Warning,
                "DTB prop {} expected to have at least {} cells but has {}",
                self,
                cells.end,
                self.blob.len() / 4
            );
        }
        let mut value = 0u128;
        for cell in cells {
            value <<= 32;
            value |= (self.blob[cell * 4 + 3] as u128) << 0;
            value |= (self.blob[cell * 4 + 2] as u128) << 8;
            value |= (self.blob[cell * 4 + 1] as u128) << 16;
            value |= (self.blob[cell * 4 + 0] as u128) << 24;
        }
        Some(value)
    }

    /// Read this prop as some integer.
    pub fn read_uint(&self) -> Option<u128> {
        self.read_uint_cells(0..self.blob.len().div_ceil(4))
    }
}

impl Display for DtbProp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.parent().fmt(f)?;
        f.write_str(" prop ")?;
        f.write_str(&self.name)?;
        Ok(())
    }
}
