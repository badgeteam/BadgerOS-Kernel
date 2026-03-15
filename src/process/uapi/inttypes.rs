// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#![allow(non_camel_case_types)]

use core::ffi::{c_int, c_long, c_uint};

pub type blkcnt_t = i64;
pub type blksize_t = u64;
pub type clockid_t = i64;
pub type dev_t = u64;
pub type fsblkcnt_t = blkcnt_t;
pub type fsblksize_t = blksize_t;
pub type fsfilcnt_t = u64;
pub type nlink_t = u64;
pub type pid_t = c_int;
pub type uid_t = c_uint;
pub type suseconds_t = i64;
pub type useconds_t = u64;
pub type clock_t = c_long;
pub type gid_t = c_uint;
pub type off_t = i64;
pub type ino_t = i64;
pub type mode_t = c_uint;
pub type time_t = c_long;
