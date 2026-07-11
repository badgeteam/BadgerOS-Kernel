// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::fmt::Display;

use alloc::sync::Arc;
use dtb::DtbNode;

use crate::{
    bindings::error::EResult,
    dev2::{Device, class::atactl::AtaCtlDevice},
    mem::dma::DmaTarget,
};

use super::{Bus, BusBase};

/// ATA command types.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Command {
    Nop = 0x00,
    DataSetMgmt = 0x06,
    DevReset = 0x08,
    ReqSenseDataExt = 0x0b,
    ReadDma = 0xc8,
    ReadDmaExt = 0x25,
    WriteDma = 0xca,
    WriteDmaExt = 0x35,
    FlushCache = 0xe7,
    FlushCacheExt = 0xea,
    IdentDev = 0xec,
}

/// ATA storage bus.
pub struct AtaBus {
    base: BusBase,
    /// Some form of ATA controller device.
    ctrl: Arc<dyn AtaCtlDevice>,
    /// Controller-specific port ID.
    port: u32,
}

impl AtaBus {
    pub fn new(ctrl: Arc<dyn AtaCtlDevice>, port: u32) -> Self {
        Self {
            base: BusBase::new(),
            ctrl,
            port,
        }
    }

    pub fn ata_cmd(
        &self,
        cmd: Command,
        ctrl: u8,
        sec_count: u16,
        feature: u16,
        lba: u64,
        data_offset: u64,
        data_length: u64,
        data: Option<&dyn DmaTarget>,
    ) -> EResult<()> {
        self.ctrl.ata_cmd(
            self.port,
            cmd,
            ctrl,
            sec_count,
            feature,
            lba,
            data_offset,
            data_length,
            data,
        )
    }
}

impl Bus for AtaBus {
    fn base(&self) -> &BusBase {
        &self.base
    }

    fn parent_device(&self) -> Option<Arc<dyn Device>> {
        Some(self.ctrl.clone())
    }

    unsafe fn install_irq(&self, _irq_id: u128, _device: *const dyn Device) -> EResult<()> {
        unreachable!()
    }

    unsafe fn uninstall_irq(&self, _irq_id: u128, _device: *const dyn Device) {
        unreachable!()
    }

    fn dtb_node(&self) -> Option<&'static DtbNode> {
        None
    }
}

impl Display for AtaBus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{} port {}", &self.ctrl, self.port))
    }
}
