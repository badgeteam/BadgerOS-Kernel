// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::str;

use alloc::string::String;

/// Trait that contains the common interface to [`StaticString`] and [`String`].
pub trait StringLike {
    /// Clear the contents of this buffer.
    fn clear(&mut self);

    #[must_use]
    /// Append a character to the end of this string.
    /// Returns whether the character as appended successfully.
    fn push(&mut self, value: char) -> bool;

    #[must_use]
    /// Get the length, in bytes, of this string.
    fn len(&self) -> usize;
}

impl StringLike for String {
    fn clear(&mut self) {
        self.clear();
    }

    fn push(&mut self, value: char) -> bool {
        if let Err(_) = self.try_reserve(1) {
            false
        } else {
            self.push(value);
            true
        }
    }

    fn len(&self) -> usize {
        self.len()
    }
}

#[derive(Clone, Copy)]
/// A UTF-8 string with a static buffer.
pub struct StaticString<const LENGTH: usize> {
    buf: [u8; LENGTH],
    len: usize,
}

impl<const LENGTH: usize> AsMut<str> for StaticString<LENGTH> {
    fn as_mut(&mut self) -> &mut str {
        unsafe { str::from_utf8_unchecked_mut(&mut self.buf[..self.len]) }
    }
}

impl<const LENGTH: usize> AsRef<str> for StaticString<LENGTH> {
    fn as_ref(&self) -> &str {
        unsafe { str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

impl<const LENGTH: usize> StaticString<LENGTH> {
    /// Create an empty string.
    pub const fn new() -> Self {
        Self {
            buf: [0u8; LENGTH],
            len: 0,
        }
    }

    #[must_use]
    /// Get a reference to the buffer.
    pub fn buf(&self) -> &[u8] {
        &self.buf[0..self.len]
    }

    #[must_use]
    /// Get a mutable reference to the buffer.
    pub fn buf_mut(&mut self) -> &mut [u8] {
        &mut self.buf[0..self.len]
    }
}

impl<const LENGTH: usize> StringLike for StaticString<LENGTH> {
    fn clear(&mut self) {
        self.len = 0;
    }

    fn push(&mut self, value: char) -> bool {
        if self.len + value.len_utf8() > LENGTH {
            return false;
        }
        value.encode_utf8(&mut self.buf[self.len..]);
        self.len += value.len_utf8();
        true
    }

    fn len(&self) -> usize {
        self.len
    }
}
