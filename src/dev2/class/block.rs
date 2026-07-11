// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{
    bindings::error::EResult,
    dev2::Device,
    kernel::sync::mutex::{Mutex, SharedMutexGuard},
    mem::{dma::DmaTarget, pagecache::PageCache},
    process::usercopy::{UserSlice, UserSliceMut},
};

/// Base block device struct; intended for use by implementers of [`BlockDevice`].
pub struct BlockDeviceBase {
    cache: Mutex<Option<PageCache>>,
}

/// Non-volatile storage device with a power-of-2 block size.
///
/// Devices with no media cannot be meaningfully accessed and fail calls with `ENOMEDIUM`.
/// The standard way to check for media is to call [`BlockDevice::identify`].
pub trait BlockDevice: Device {
    /// Get the block device base struct.
    fn block_base(&self) -> &BlockDeviceBase;

    /// Get block device information.
    fn identify_uncached(&self) -> EResult<BlockIdent>;

    /// Read uncached data blocks; bypasses the built-in page cache.
    fn read_blocks_uncached(&self, lba: u64, rdata: &dyn DmaTarget) -> EResult<()>;

    /// Write uncached data blocks; bypasses the built-in page cache.
    fn write_blocks_uncached(&self, lba: u64, rdata: &dyn DmaTarget) -> EResult<()>;
}

impl dyn BlockDevice {
    /// Allocate the page cache if necessary and borrow it.
    fn get_cache<'a>(&'a self) -> EResult<SharedMutexGuard<'a, PageCache>> {
        todo!()
    }

    /// Get the last cached block size exponent.
    /// Returns meaningless values if [`Self::identify`] hasn't run or there is no media.
    pub fn block_size_exp(&self) -> u8 {
        todo!()
    }

    /// Get the last cached block count.
    /// Returns meaningless values if [`Self::identify`] hasn't run or there is no media.
    pub fn block_count(&self) -> u64 {
        todo!()
    }

    /// Get block device information.
    pub fn identify(&self) -> EResult<BlockIdent> {
        todo!()
    }

    /// Read bytes through the cache.
    #[inline(always)]
    pub fn readk_bytes(&self, addr: u64, rdata: &mut [u8]) -> EResult<()> {
        self.read_bytes(addr, UserSliceMut::new_kernel_mut(rdata))
    }

    /// Read bytes through the cache.
    pub fn read_bytes(&self, addr: u64, rdata: UserSliceMut<u8>) -> EResult<()> {
        todo!()
    }

    /// Write bytes through the cache.
    #[inline(always)]
    pub fn writek_bytes(&self, addr: u64, wdata: &[u8]) -> EResult<()> {
        self.write_bytes(addr, UserSlice::new_kernel(wdata))
    }

    /// Write bytes through the cache.
    pub fn write_bytes(&self, addr: u64, wdata: UserSlice<u8>) -> EResult<()> {
        todo!()
    }

    /// Write zeroes through the cache.
    pub fn write_zeroes(&self, addr: u64, len: u64) -> EResult<()> {
        todo!()
    }

    /// Sync bytes from the cache to disk.
    /// If `flush` is `true`, removes cached reads as well.
    pub fn sync_bytes(&self, addr: u64, len: u64, flush: bool) -> EResult<()> {
        todo!()
    }

    /// Sync all data from the cache to disk.
    /// If `flush` is `true`, removes cached reads as well.
    pub fn sync_all(&self, flush: bool) -> EResult<()> {
        todo!()
    }
}

/// Block device identification and metadata.
pub struct BlockIdent {
    /// Log-base 2 of the block size.
    pub block_size_exp: u8,
    /// Total block count.
    pub block_count: u64,
}
