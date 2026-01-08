// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ffi::{CStr, c_char},
    ptr::{null_mut, slice_from_raw_parts},
};

use crate::bindings::{
    self,
    raw::{dtb_node_t, dtb_prop_t, uintmax_t},
};

/// Represents a device tree node.
pub struct DtbNode(dtb_node_t);

impl DtbNode {
    pub fn parent(&self) -> Option<&DtbNode> {
        unsafe { core::mem::transmute(self.0.parent) }
    }
    pub fn name(&self) -> &str {
        unsafe { CStr::from_ptr(self.0.name) }.to_str().unwrap()
    }
    pub fn phandle(&self) -> Option<u32> {
        self.0.has_phandle.then_some(self.0.phandle)
    }
    pub fn get_prop(&self, name: &str) -> Option<&DtbProp> {
        unsafe {
            core::mem::transmute(bindings::raw::dtb_get_prop_l(
                null_mut(),
                &raw const self.0 as *mut dtb_node_t,
                name.as_ptr() as *const c_char,
                name.len(),
            ))
        }
    }
    pub fn get_node(&self, name: &str) -> Option<&DtbNode> {
        unsafe {
            core::mem::transmute(bindings::raw::dtb_get_node_l(
                null_mut(),
                &raw const self.0 as *mut dtb_node_t,
                name.as_ptr() as *const c_char,
                name.len(),
            ))
        }
    }
    pub fn child_nodes(&self) -> &[DtbNode] {
        unsafe { &*slice_from_raw_parts(self.0.nodes as *const DtbNode, self.0.nodes_len) }
    }
    pub fn child_props(&self) -> &[DtbProp] {
        unsafe { &*slice_from_raw_parts(self.0.props as *const DtbProp, self.0.props_len) }
    }
}

/// Represents a device tree property.
pub struct DtbProp(dtb_prop_t);

impl DtbProp {
    pub fn name(&self) -> &str {
        unsafe { CStr::from_ptr(self.0.name) }.to_str().unwrap()
    }
    pub fn bytes(&self) -> &[u8] {
        unsafe { &*slice_from_raw_parts(self.0.content as *const u8, self.0.content_len as usize) }
    }
    pub fn read_uint(&self) -> uintmax_t {
        unsafe {
            bindings::raw::dtb_prop_read_uint(null_mut(), &raw const self.0 as *mut dtb_prop_t)
        }
    }
    pub fn read_cell(&self, cell: u32) -> u32 {
        unsafe {
            bindings::raw::dtb_prop_read_cell(
                null_mut(),
                &raw const self.0 as *mut dtb_prop_t,
                cell,
            )
        }
    }
    pub fn read_cells(&self, cell: u32, len: u32) -> uintmax_t {
        unsafe {
            bindings::raw::dtb_prop_read_cells(
                null_mut(),
                &raw const self.0 as *mut dtb_prop_t,
                cell,
                len,
            )
        }
    }
}
