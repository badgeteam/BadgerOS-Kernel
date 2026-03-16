// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::c_int;

/// ID of calling process.
pub const GETID_PID: c_int = 0;
/// ID of calling process' parent.
pub const GETID_PPID: c_int = 1;
/// ID of calling thread.
pub const GETID_TID: c_int = 2;
/// User ID of calling process.
pub const GETID_UID: c_int = 3;
/// Effective user ID of calling process.
pub const GETID_EUID: c_int = 4;
/// Group ID of calling process.
pub const GETID_GID: c_int = 3;
/// Effective group ID of calling process.
pub const GETID_EGID: c_int = 4;
