// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::vec::Vec;

use crate::{
    bindings::error::EResult,
    dev2::Device,
    kernel::sync::waitlist::Waitlist,
    process::usercopy::{UserSlice, UserSliceMut},
};

/// A character device (e.g. 16550 UART, /dev/null, etc).
pub trait CharDevice: Device {
    /// Is this a TTY?
    fn is_tty(&self) -> bool {
        true
    }

    /// Get current polling status flags.
    fn poll(&self) -> u32;

    /// Collect waitlists for the requested poll interest flags.
    fn poll_waitlists<'a>(&'a self, interest: u32, collect: &mut Vec<&'a Waitlist>) -> EResult<()>;

    /// Read raw bytes from this device (ignoring termios).
    /// Upon (partial) success, returns how many bytes were read.
    fn read_raw(&self, rdata: UserSliceMut<u8>, nonblock: bool) -> EResult<usize>;

    /// Write raw bytes to this device (ignoring termios).
    /// Upon (partial) success, returns how many bytes were written.
    fn write_raw(&self, wdata: UserSlice<u8>, nonblock: bool) -> EResult<usize>;
}
