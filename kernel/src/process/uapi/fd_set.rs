// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#![allow(non_camel_case_types)]

use crate::process::usercopy::UserCopyable;

pub const FD_SETSIZE: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq)]
struct fd_set {
    pub fds_bits: [u8; FD_SETSIZE / 8],
}
unsafe impl UserCopyable for fd_set {}
