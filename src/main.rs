// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#![no_std]
#![no_main]
#![feature(allocator_api)]
#![feature(formatting_options)]
#![allow(dead_code)]
#![allow(unused_macros)]
#![feature(unsafe_cell_access)]
#![feature(error_generic_member_access)]
#![feature(str_from_raw_parts)]
#![feature(negative_impls)]
#![feature(ptr_metadata)]
#![feature(box_vec_non_null)]
#![feature(try_with_capacity)]
#![feature(str_from_utf16_endian)]
#![feature(ascii_char)]
#![feature(atomic_try_update)]
#![feature(try_blocks)]
#![feature(likely_unlikely)]
#![feature(map_try_insert)]
#![feature(iterator_try_collect)]
#![feature(generic_const_exprs)]
#![feature(int_lowest_highest_one)]
#![feature(generic_atomic)]
#![feature(btree_cursors)]
#![feature(linked_list_cursors)]
#![feature(bigint_helper_methods)]
#![feature(int_roundings)]
#![feature(try_trait_v2)]
#![feature(transmutability)]
#![feature(vec_try_remove)]
#![feature(arc_is_unique)]
#![feature(debug_closure_helpers)]
#![feature(unsize)]
#![feature(macro_metavar_expr_concat)]
#![feature(iter_collect_into)]
#![feature(downcast_unchecked)]
#![feature(const_trait_impl)]
#![feature(const_cmp)]
#![feature(cfg_select)]
#![feature(core_intrinsics)]
#![allow(internal_features)]
#![feature(adt_const_params)]

#[macro_use]
extern crate alloc;
extern crate chrono;

#[macro_use]
pub mod util;

pub mod arch;
pub mod boot;
pub mod device;
pub mod driver;
pub mod error;
pub mod except;
pub mod filesystem;
pub mod kcore;
pub mod mem;
pub mod misc;
pub mod process;

#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(unused)]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(non_upper_case_globals)]
pub mod abi {
    include!(concat!(env!("OUT_DIR"), "/abi.rs"));
}

#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(unused)]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(non_upper_case_globals)]
pub mod uacpi_sys {
    include!(concat!(env!("OUT_DIR"), "/uacpi.rs"));
}

pub use util::log::*;
