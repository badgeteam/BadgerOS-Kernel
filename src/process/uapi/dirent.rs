// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#![allow(non_camel_case_types)]

use core::ffi::c_ushort;

use bytemuck_derive::{AnyBitPattern, NoUninit};

use crate::{filesystem::NAME_MAX, process::usercopy::UserCopyable};

pub const DT_UNKNOWN: u8 = 0;
pub const DT_FIFO: u8 = 1;
pub const DT_CHR: u8 = 2;
pub const DT_DIR: u8 = 4;
pub const DT_BLK: u8 = 6;
pub const DT_REG: u8 = 8;
pub const DT_LNK: u8 = 10;
pub const DT_SOCK: u8 = 12;
pub const DT_WHT: u8 = 14;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct dirent {
    pub d_off: i64,
    pub d_reclen: c_ushort,
    pub d_type: u8,
    pub d_name: [u8; NAME_MAX + 1],
}
unsafe impl UserCopyable for dirent {}

#[repr(C, packed)]
#[derive(Clone, Copy, NoUninit, AnyBitPattern)]
pub struct dirent_headeronly {
    pub d_off: i64,
    pub d_reclen: c_ushort,
    pub d_type: u8,
}
unsafe impl UserCopyable for dirent_headeronly {}
