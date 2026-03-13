// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ffi::c_char,
    fmt::{Display, Write},
    ops::Range,
    ptr::slice_from_raw_parts,
};

use alloc::{boxed::Box, collections::btree_map::BTreeMap, string::String};
use spec::*;

use crate::bindings::{self, log::LogLevel};

mod c_api;
mod spec;

/// Loaded device tree structure.
pub struct Dtb {
    /// DTB root node.
    pub root: DtbNode,
    /// Map from phandle to node.
    by_phandle: BTreeMap<u32, *const DtbNode>,
}

impl Dtb {
    pub const MIN_SUPPORTED: u32 = 16;
    pub const MAX_SUPPORTED: u32 = 17;

    /// Parse DTB from an FDT pointer.
    /// # Panics
    /// - If the FDT is malformed.
    pub unsafe fn parse(fdt: *const ()) -> Self {
        let mut by_phandle = BTreeMap::<u32, *const DtbNode>::new();
        const NULL_STR: *const str = core::ptr::from_raw_parts(0 as *const (), 0);
        let mut root = DtbNode {
            name: NULL_STR,
            parent: 0 as *const DtbNode,
            phandle: None,
            nodes: BTreeMap::new(),
            props: BTreeMap::new(),
        };

        unsafe {
            // Validate header.
            let header = (*(fdt as *const FdtHeader)).from_be();
            assert!(
                header.magic == FdtHeader::MAGIC,
                "FDT header has invalid magic"
            );
            assert!(
                16 <= header.compat_version && header.compat_version <= 17,
                "FDT uses incompatible version {} (supported are {}-{})",
                header.compat_version,
                Self::MIN_SUPPORTED,
                Self::MAX_SUPPORTED
            );
            let struct_ = fdt.byte_add(header.struct_offset as usize);
            let string = fdt.byte_add(header.string_offset as usize);

            let mut node = &raw mut root;
            let mut index = 0usize;
            loop {
                let token = u32::from_be(*(struct_ as *const u32).add(index));
                index += 1;
                if token == FDT_BEGIN_NODE {
                    // Beginning of FDT node; extract name.
                    let name_ptr = struct_.byte_add(index * 4) as *const u8;
                    let name_len = bindings::raw::strlen(name_ptr as *const c_char);
                    index += (name_len + 1).div_ceil(4);
                    let name = str::from_utf8_unchecked(&*slice_from_raw_parts(name_ptr, name_len));

                    // Assert that the name isn't taken.
                    assert!(
                        (*node).nodes.get(name).is_none() && (*node).props.get(name).is_none(),
                        "FDT node with duplicate name"
                    );

                    // Insert new empty node in parent.
                    (*node).nodes.insert(
                        name.into(),
                        DtbNode {
                            name: NULL_STR,
                            parent: node,
                            phandle: None,
                            nodes: BTreeMap::new(),
                            props: BTreeMap::new(),
                        },
                    );

                    // Now that it's in the tree, set its name.
                    let (name, new_node) = (*node).nodes.get_key_value(name).unwrap();
                    node = new_node as *const DtbNode as *mut DtbNode;
                    (*node).name = name.as_ref();
                } else if token == FDT_END_NODE {
                    // End of FDT node.
                    assert!(!(*node).parent.is_null(), "Unexepcted FDT_END_NODE token");
                    node = (*node).parent as *mut DtbNode;
                } else if token == FDT_PROP {
                    // FDT prop; get name and length.
                    let len = u32::from_be(*(struct_ as *const u32).add(index));
                    let nameoff = u32::from_be(*(struct_ as *const u32).add(index + 1));
                    index += 2;
                    let name_ptr = string.byte_add(nameoff as usize) as *const u8;
                    let name_len = bindings::raw::strlen(name_ptr as *const c_char);
                    let name = str::from_utf8_unchecked(&*slice_from_raw_parts(name_ptr, name_len));

                    // Assert that the name isn't taken.
                    assert!(
                        (*node).nodes.get(name).is_none() && (*node).props.get(name).is_none(),
                        "FDT prop with duplicate name"
                    );

                    // Read prop value.
                    let blob = Box::<[u8]>::from(&*slice_from_raw_parts(
                        struct_.byte_add(index * 4) as *const u8,
                        len as usize,
                    ));
                    index += len.div_ceil(4) as usize;

                    // Insert new prop.
                    (*node).props.insert(
                        name.into(),
                        DtbProp {
                            name: NULL_STR,
                            parent: node,
                            blob,
                        },
                    );
                    let (name, prop) = (*node).props.get_key_value(name).unwrap();
                    let prop = prop as *const DtbProp as *mut DtbProp;
                    (*prop).name = name.as_ref();

                    // Automatically set phandles.
                    if *name == *"phandle" {
                        let phandle = (&*prop).read_cell(0);
                        match phandle {
                            Some(phandle) => {
                                (*node).phandle = Some(phandle);
                                by_phandle.insert(phandle, node);
                            }
                            None => {
                                logkf!(LogLevel::Warning, "Ignored malformed phandle in {}", &*node)
                            }
                        }
                    }
                } else if token == FDT_NOP {
                    // Ignored.
                } else if token == FDT_END {
                    // End of the FDT structure block.
                    assert!(node == &raw mut root, "Unexpected FDT_END token");
                    break;
                } else {
                    panic!("Invalid FDT token")
                }
            }
        }

        Dtb { root, by_phandle }
    }

    /// Get a node by its phandle.
    pub fn node_by_phandle(&self, phandle: u32) -> Option<&DtbNode> {
        self.by_phandle.get(&phandle).map(|x| unsafe { &**x })
    }
}

/// Device tree node.
pub struct DtbNode {
    /// This node's name.
    name: *const str,
    /// Parent node, if any.
    parent: *const DtbNode,
    /// Cached phandle, if any.
    pub phandle: Option<u32>,
    /// Child nodes and props.
    pub nodes: BTreeMap<String, DtbNode>,
    /// Child props.
    pub props: BTreeMap<String, DtbProp>,
}

impl DtbNode {
    /// Get the node's name.
    pub fn name(&self) -> &str {
        if self.name.is_null() {
            return "";
        }
        unsafe { &*self.name }
    }

    /// Get the parent node.
    pub fn parent(&self) -> Option<&DtbNode> {
        if self.parent.is_null() {
            return None;
        }
        Some(unsafe { &*self.parent })
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
            f.write_str(cur.name())?;
            f.write_char('/')?;
        }
        f.write_str(self.name())?;

        Ok(())
    }
}

/// Device tree property.
pub struct DtbProp {
    /// This prop's name.
    name: *const str,
    /// Parent node, if any.
    parent: *const DtbNode,
    /// Binary value.
    pub blob: Box<[u8]>,
}

impl DtbProp {
    /// Get the prop's name.
    pub fn name(&self) -> &str {
        unsafe { &*self.name }
    }

    /// Get the parent node.
    pub fn parent(&self) -> &DtbNode {
        unsafe { &*self.parent }
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
        f.write_str(self.name())?;
        Ok(())
    }
}
