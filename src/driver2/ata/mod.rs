// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    any::{Any, TypeId},
    fmt::Display,
};

use alloc::{boxed::Box, sync::Arc};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    dev2::{
        self, Device, DeviceBase,
        bus::{
            Bus, BusResv,
            ata::{AtaBus, Command},
        },
        class::block::{BlockDevice, BlockDeviceBase, BlockIdent},
        driver::Driver,
    },
    device_get_trait_vtable,
    mem::dma::{DmaFromRef, DmaTarget},
    register_kmodule,
};

/// ATA block device.
pub struct AtaBlockDevice {
    base: DeviceBase,
    block_base: BlockDeviceBase,
    bus: BusResv<AtaBus>,
}

impl Display for AtaBlockDevice {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.bus.fmt(f)
    }
}

impl Device for AtaBlockDevice {
    fn base(&self) -> &DeviceBase {
        &self.base
    }

    fn interrupt(&self, _id: u128) -> bool {
        unreachable!()
    }

    device_get_trait_vtable!(BlockDevice);
}

impl BlockDevice for AtaBlockDevice {
    fn block_base(&self) -> &BlockDeviceBase {
        &self.block_base
    }

    fn identify_uncached(&self) -> EResult<BlockIdent> {
        let mut ident = Box::<[u16; 512]>::new_uninit();
        self.bus.take()?.ata_cmd(
            Command::IdentDev,
            1 << 6,
            0,
            0,
            0,
            0,
            1024,
            Some(DmaFromRef::from_mut(ident.as_mut())),
        )?;
        let ident = unsafe { ident.assume_init() };

        let supports_48bit = ident[83] & (1 << 10) != 0;
        let block_size_exp;
        if ident[106] & (1 << 14) == 0 {
            block_size_exp = 9; // 512 bytes
        } else {
            let block_size = ident[117] as u64 + (ident[118] as u64) << 16;
            if block_size == 0 {
                block_size_exp = 9; // 512 bytes
            } else {
                block_size_exp = block_size.trailing_zeros() as u8;
            }
        }
        let block_count = (ident[100] as u64)
            + ((ident[101] as u64) << 16)
            + ((ident[102] as u64) << 32)
            + ((ident[103] as u64) << 48);

        logkf!(
            LogLevel::Info,
            "{}: 48-bit: {}; sec. size: {}; sec. count: {}",
            self,
            if supports_48bit { 'y' } else { 'n' },
            1u64 << block_size_exp,
            block_count
        );

        Ok(BlockIdent {
            block_size_exp,
            block_count,
            addr_width: if supports_48bit { 48 } else { 24 },
        })
    }

    fn read_blocks_uncached(
        &self,
        mut lba: u64,
        mut data_offset: u64,
        data_length: u64,
        rdata: &dyn DmaTarget,
    ) -> EResult<()> {
        if !rdata.allow_scatter() {
            return Err(Errno::EINVAL);
        }

        let self_blk = self as &dyn BlockDevice;
        let cmd;
        if self_blk.addr_width() >= 48 {
            cmd = Command::ReadDmaExt;
        } else {
            cmd = Command::ReadDma;
        }

        let block_size_exp = self_blk.block_size_exp();
        let mut sec_count = data_length >> block_size_exp;

        while sec_count > 0 {
            let max = sec_count.min(u16::MAX as u64) as u16;
            self.bus.take()?.ata_cmd(
                cmd,
                0,
                max,
                0,
                lba,
                data_offset,
                (max as u64) << block_size_exp,
                Some(rdata),
            )?;

            lba += max as u64;
            data_offset += (max as u64) << block_size_exp;
            sec_count -= max as u64;
        }

        Ok(())
    }

    fn write_blocks_uncached(
        &self,
        mut lba: u64,
        mut data_offset: u64,
        data_length: u64,
        wdata: &dyn DmaTarget,
    ) -> EResult<()> {
        if !wdata.allow_gather() {
            return Err(Errno::EINVAL);
        }

        let self_blk = self as &dyn BlockDevice;
        let cmd;
        if self_blk.addr_width() >= 48 {
            cmd = Command::ReadDmaExt;
        } else {
            cmd = Command::ReadDma;
        }

        let block_size_exp = self_blk.block_size_exp();
        let mut sec_count = data_length >> block_size_exp;

        while sec_count > 0 {
            let max = sec_count.min(u16::MAX as u64) as u16;
            self.bus.take()?.ata_cmd(
                cmd,
                0,
                max,
                0,
                lba,
                data_offset,
                (max as u64) << block_size_exp,
                Some(wdata),
            )?;

            lba += max as u64;
            data_offset += (max as u64) << block_size_exp;
            sec_count -= max as u64;
        }

        Ok(())
    }
}

/// ATA block device driver.
pub struct AtaBlockDriver;

impl Driver for AtaBlockDriver {
    fn name(&self) -> &str {
        "ata-block"
    }

    fn match_(&self, bus: &dyn Bus) -> bool {
        (bus as &dyn Any).type_id() == TypeId::of::<AtaBus>()
    }

    unsafe fn probe(&self, bus: BusResv<dyn Bus>) -> EResult<Arc<dyn Device>> {
        let bus = bus.downcast::<AtaBus>().unwrap();

        Ok(Arc::try_new(AtaBlockDevice {
            base: DeviceBase::new(),
            block_base: BlockDeviceBase::new(),
            bus,
        })?)
    }
}

register_kmodule!("ata-block", || dev2::registry::register_driver(
    &AtaBlockDriver
));
