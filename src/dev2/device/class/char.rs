// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{
    bindings::error::EResult,
    dev2::device::Device,
    kernel::sync::waitlist::Waitlist,
    process::usercopy::{UserSlice, UserSliceMut},
};

/// A character device (e.g. 16550 UART, /dev/null, etc).
pub trait CharDevice: Device {
    /// Get a copy of the underlying read waitlist.
    /// Used to implement `select`.
    fn read_waitlist(&self) -> Option<&Waitlist>;

    /// Get a copy of the underlying read waitlist.
    /// Used to implement `select`.
    fn write_waitlist(&self) -> Option<&Waitlist>;

    /// Poll for available read and/or write space.
    /// Used to implement `select`.
    fn poll(&self, read: bool, write: bool) -> bool;

    /// Read raw bytes from this device (ignoring termios).
    /// Upon (partial) success, returns how many bytes were read.
    fn read_raw(&self, rdata: UserSliceMut<u8>, nonblock: bool) -> EResult<usize>;

    /// Write raw bytes to this device (ignoring termios).
    /// Upon (partial) success, returns how many bytes were written.
    fn write_raw(&self, wdata: UserSlice<u8>, nonblock: bool) -> EResult<usize>;
}
