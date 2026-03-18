// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::process::usercopy::UserCopyable;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NameField(pub [u8; 65]);

impl NameField {
    pub const fn assign_bytes(&mut self, data: &[u8]) {
        let mut i = 0;
        while i < data.len() {
            self.0[i] = data[i];
            i += 1;
        }
    }

    pub const fn assign(&mut self, data: &str) {
        self.assign_bytes(data.as_bytes());
    }
}

impl Default for NameField {
    fn default() -> Self {
        Self([0; _])
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct utsname {
    /// Operating system name.
    pub sysname: NameField,
    /// Network node name.
    pub nodename: NameField,
    /// Release number and variant.
    pub release: NameField,
    /// Build date and metadata.
    pub version: NameField,
    /// Architecture name.
    pub machine: NameField,
    /// NIS or YP domain name.
    pub domainname: NameField,
}
unsafe impl UserCopyable for utsname {}
