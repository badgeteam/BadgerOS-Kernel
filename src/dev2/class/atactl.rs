// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{
    bindings::error::EResult,
    dev2::{Device, bus::ata::Command},
    mem::dma::DmaTarget,
};

/// Devices that expose one or more ATA buses.
pub trait AtaCtlDevice: Device {
    fn ata_cmd(
        &self,
        port: u32,
        cmd: Command,
        ctrl: u8,
        sec_count: u16,
        feature: u16,
        lba: u64,
        data: Option<&dyn DmaTarget>,
    ) -> EResult<()>;
}
