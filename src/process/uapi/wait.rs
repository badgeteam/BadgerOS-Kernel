// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

pub const WNOHANG: i32 = 1;
pub const WUNTRACED: i32 = 2;
pub const WSTOPPED: i32 = 2;
pub const WEXITED: i32 = 4;
pub const WCONTINUED: i32 = 8;
pub const WNOWAIT: i32 = 0x01000000;

pub const WCOREFLAG: i32 = 0x80;

pub const fn w_exited(code: i32) -> i32 {
    (code & 0xff) << 8
}

pub const fn w_signalled(sig: i32) -> i32 {
    ((sig & 0xff) << 16) | 0x40
}

pub const fn w_stopped(sig: i32) -> i32 {
    ((sig & 0xff) << 16) | 0x20
}

pub const fn w_continued(sig: i32) -> i32 {
    ((sig & 0xff) << 16) | 0x20
}

pub const fn w_if_exited(status: i32) -> bool {
    (status & 0xff) == 0
}

pub const fn w_if_signalled(status: i32) -> bool {
    (status & 0x40) != 0
}

pub const fn w_if_stopped(status: i32) -> bool {
    (status & 0x20) != 0
}

pub const fn w_if_continued(status: i32) -> bool {
    (status & 0x10) != 0
}
