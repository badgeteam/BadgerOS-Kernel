// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::sync::Arc;

use crate::dev2::{
    bus::mmio::MmioBus,
    device::{Device, DeviceMeta},
};

pub struct Ns16550aDevice {
    base: DeviceMeta,
    bus: Arc<MmioBus>,
}

impl Device for Ns16550aDevice {
    fn base(&self) -> &DeviceMeta {
        &self.base
    }
}
