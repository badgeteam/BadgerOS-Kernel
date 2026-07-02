// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#![no_std]
#![feature(allocator_api)]

extern crate alloc;
extern crate core;

use core::{
    error::Error,
    fmt::{Debug, Display, Write},
    ops::Deref,
    ptr::null,
};

use alloc::{boxed::Box, collections::btree_map::BTreeMap, string::String, vec::Vec};

pub mod spec;
use spec::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtbError {
    /// Invalid FDT.
    Invalid,
    /// Out of memory.
    NoMemory,
}

impl Display for DtbError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <Self as Debug>::fmt(self, f)
    }
}

impl Error for DtbError {}

/// Loaded device tree structure.
pub struct Dtb {
    /// DTB root node.
    root: Box<DtbNode>,
    /// Map from phandle to node.
    by_phandle: BTreeMap<u32, *const DtbNode>,
    /// DTB path aliases.
    pub aliases: BTreeMap<String, String>,
}
unsafe impl Send for Dtb {}
unsafe impl Sync for Dtb {}

impl Dtb {
    pub const MIN_SUPPORTED: u32 = 16;
    pub const MAX_SUPPORTED: u32 = 17;

    /// Parse DTB from an FDT pointer.
    /// # Panics
    /// - If the FDT is malformed
    /// - If a `phandle`, `#address-cells`, `#size-cells` or `#interrupt-cells` property is not exactly one cell
    /// - If a name is not valid UTF-8
    /// - If an alias is not valid UTF-8
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

        let mut aliases = BTreeMap::new();
        if let Some(node) = root.node("aliases") {
            for prop in node.props.values() {
                let string = prop.as_string().expect("Alias must be one valid string");
                aliases.insert(prop.name.clone(), string.into());
            }
        }

        Self {
            root,
            by_phandle,
            aliases,
        }
    }

    /// DTB root node.
    pub fn root(&self) -> &DtbNode {
        &self.root
    }

    /// Get a DTB node from the root by name.
    pub fn node(&self, name: &str) -> Option<&DtbNode> {
        self.root.node(name)
    }

    /// Get a node by its phandle.
    pub fn node_by_phandle(&self, phandle: u32) -> Option<&DtbNode> {
        self.by_phandle.get(&phandle).map(|x| unsafe { &**x })
    }
}

impl Dtb {
    fn pindent(depth: usize, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for _ in 0..depth {
            f.write_str("    ")?;
        }
        Ok(())
    }

    fn display_impl_prop(
        &self,
        prop: &DtbProp,
        depth: usize,
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        Self::pindent(depth, f)?;
        f.write_str(&prop.name)?;
        if prop.name.starts_with("#")
            && prop.name.ends_with("-cells")
            && let Some(value) = prop.read_u32()
        {
            f.write_fmt(format_args!(" = <{}>;\n", value))?;
            return Ok(());
        } else if prop.blob.is_empty() {
            f.write_str(";\n")?;
            return Ok(());
        }

        f.write_str(" = ")?;

        let mut is_strings = true;
        let mut prev = 0u8;
        for &c in &prop.blob {
            if c == 0 {
                if prev == 0 {
                    is_strings = false;
                    break;
                }
            } else if c < 0x20 || c > 0x7f {
                is_strings = false;
                break;
            }
            prev = c;
        }
        is_strings &= prop.blob.last() == Some(&0);

        if is_strings {
            f.write_char('\"')?;
            let strings = prop.blob[..prop.blob.len() - 1].split(|&x| x == 0);
            let mut sep = false;
            for string in strings {
                if sep {
                    f.write_str("\", \"")?;
                }
                // SAFETY: ASCII is a strict subset of UTF-8.
                f.write_str(unsafe { str::from_utf8_unchecked(string) })?;
                sep = true;
            }
            f.write_char('\"')?;
        } else if let Ok(cells) = bytemuck::try_cast_slice::<u8, u32>(&prop.blob) {
            // Cells.
            f.write_char('<')?;
            for i in 0..cells.len() {
                if i > 0 {
                    f.write_char(' ')?;
                }
                f.write_fmt(format_args!("0x{:08x}", u32::from_be(cells[i])))?;
            }
            f.write_char('>')?;
        } else {
            // Plain bytes.
            f.write_char('[')?;
            for i in 0..prop.blob.len() {
                if i > 0 {
                    f.write_char(' ')?;
                }
                f.write_fmt(format_args!("0x{:02x}", prop.blob[i]))?;
            }
            f.write_char(']')?;
        }

        f.write_str(";\n")?;

        Ok(())
    }

    fn display_impl_node(
        &self,
        name: &str,
        node: &DtbNode,
        depth: usize,
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        Self::pindent(depth, f)?;
        f.write_str(name)?;
        f.write_str(" {\n")?;

        for prop in node.props.values() {
            self.display_impl_prop(prop, depth + 1, f)?;
        }
        for child in node.nodes.values() {
            self.display_impl_node(&child.name, child, depth + 1, f)?;
        }

        Self::pindent(depth, f)?;
        f.write_str("}\n")?;

        Ok(())
    }
}

impl Display for Dtb {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.display_impl_node("/", &self.root, 0, f)?;
        Ok(())
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
    /// Cached #size-cells, if any.
    pub size_cells: Option<u32>,
    /// Cached #address-cells, if any.
    pub addr_cells: Option<u32>,
    /// Cached #interrupt-cells, if any.
    pub irq_cells: Option<u32>,
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
            size_cells: None,
            addr_cells: None,
            irq_cells: None,
            nodes: BTreeMap::new(),
            props: BTreeMap::new(),
        });
        if !parent.is_null() {
            unsafe {
                this.size_cells = (*parent).size_cells;
                this.addr_cells = (*parent).addr_cells;
                this.irq_cells = (*parent).irq_cells;
            }
        }

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
            assert!(prop.cell_count() == Some(1), "phandle must have one cell");
            prop.read_cell(0).unwrap()
        });
        this.size_cells = this.props.get("#size-cells").map(|prop| {
            assert!(
                prop.cell_count() == Some(1),
                "#size-cells must have one cell"
            );
            prop.read_cell(0).unwrap()
        });
        this.addr_cells = this.props.get("#address-cells").map(|prop| {
            assert!(
                prop.cell_count() == Some(1),
                "#address-cells must have one cell"
            );
            prop.read_cell(0).unwrap()
        });
        this.irq_cells = this.props.get("#interrupt-cells").map(|prop| {
            assert!(
                prop.cell_count() == Some(1),
                "#interrupt-cells must have one cell"
            );
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

    /// Get a child of this node by name.
    pub fn node(&self, name: &str) -> Option<&DtbNode> {
        self.nodes.get(name).map(AsRef::as_ref)
    }

    /// Get a property from this node or its direct parent.
    pub fn inherit_prop(&self, name: &str) -> Option<&DtbProp> {
        if let Some(prop) = self.props.get(name) {
            return Some(prop);
        }
        if let Some(parent) = self.parent()
            && let Some(prop) = parent.props.get(name)
        {
            return Some(prop);
        }
        None
    }

    /// Get a property of this node by name.
    pub fn prop(&self, name: &str) -> Option<&DtbProp> {
        self.props.get(name)
    }

    /// Read a named property as one u32.
    pub fn prop_u32(&self, name: &str) -> Option<u32> {
        self.props.get(name)?.read_u32()
    }

    /// Read a named property as one u64.
    pub fn prop_u64(&self, name: &str) -> Option<u64> {
        self.props.get(name)?.read_u64()
    }

    /// Read a named property as some integer.
    pub fn prop_uint(&self, name: &str) -> Option<u128> {
        self.props.get(name)?.read_uint()
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
        f.write_char('/')?;
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

    /// Read as one NUL-terminated string.
    pub fn as_string(&self) -> Option<&str> {
        if self.blob.last() != Some(&0) {
            return None;
        }
        str::from_utf8(&self.blob[..self.blob.len() - 1]).ok()
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
        self.read_uint_cells(cell, 1).map(|x| x as u32)
    }

    /// Read `count` cells starting at cell index `start` into a boxed slice.
    pub fn read_cells(&self, start: usize, count: usize) -> Result<Box<[u32]>, DtbError> {
        let mut v = Vec::new();
        v.try_reserve(count).map_err(|_| DtbError::NoMemory)?;
        for c in 0..count {
            v.push(self.read_cell(start + c).ok_or(DtbError::Invalid)?);
        }
        Ok(v.into_boxed_slice())
    }

    /// Read this prop as some integer.
    pub fn read_uint_cells(&self, start: usize, count: usize) -> Option<u128> {
        debug_assert!(count <= 4);
        if self.blob.len() / 4 < start + count {
            return None;
        }
        let mut value = 0u128;
        for cell in start..start + count {
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
        self.read_uint_cells(0, self.blob.len().div_ceil(4))
    }

    /// Read this prop as one u32.
    pub fn read_u32(&self) -> Option<u32> {
        (self.blob.len() == 4)
            .then(|| self.read_uint_cells(0, 1))?
            .map(|x| x as u32)
    }

    /// Read this prop as one u64.
    pub fn read_u64(&self) -> Option<u64> {
        (self.blob.len() == 8)
            .then(|| self.read_uint_cells(0, 2))?
            .map(|x| x as u64)
    }
}

impl Display for DtbProp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(self.parent(), f)?;
        f.write_str(" prop ")?;
        f.write_str(&self.name)?;
        Ok(())
    }
}
