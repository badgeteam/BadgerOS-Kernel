// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ptr::NonNull;

use crate::{
    bindings::error::EResult,
    dev2::Device,
    filesystem::partition::{VolumeInfo, get_volume_info},
    kernel::sync::mutex::{Mutex, MutexGuard, SharedMutexGuard},
    mem::{
        dma::{DmaFromBuffer, DmaTarget},
        pagecache::{PageCache, Pager},
        pmm::PAddrr,
        vmm::memobject::MemObject,
    },
    process::usercopy::{UserSlice, UserSliceMut},
};

struct BlockCaches {
    cache: PageCache,
    ident: BlockIdent,
}

struct BlockVInfo {
    vinfo: Option<VolumeInfo>,
    vinfo_probed: bool,
}

/// Base block device struct; intended for use by implementers of [`BlockDevice`].
pub struct BlockDeviceBase {
    cache: Mutex<Option<BlockCaches>>,
    vinfo: Mutex<BlockVInfo>,
}

impl BlockDeviceBase {
    pub const fn new() -> Self {
        Self {
            cache: Mutex::new(None),
            vinfo: Mutex::new(BlockVInfo {
                vinfo: None,
                vinfo_probed: false,
            }),
        }
    }
}

/// Temporary [`Pager`] glue for [`BlockDevice`]; in the future, it is implemented directly by it.
struct PagerGlue<'a>(&'a dyn BlockDevice);

impl Pager for PagerGlue<'_> {
    fn memobject(&self) -> Option<NonNull<dyn MemObject>> {
        None
    }

    unsafe fn read_blocks(
        &self,
        start_block: u64,
        _block_count: usize,
        paddr: PAddrr,
        vaddr: &mut [u8],
    ) -> EResult<()> {
        self.0
            .read_blocks_uncached(start_block, 0, vaddr.len() as u64, &unsafe {
                DmaFromBuffer::from_mut(vaddr, paddr)
            })
    }

    unsafe fn write_blocks(
        &self,
        start_block: u64,
        _block_count: usize,
        paddr: PAddrr,
        vaddr: &[u8],
    ) -> EResult<()> {
        self.0
            .write_blocks_uncached(start_block, 0, vaddr.len() as u64, &unsafe {
                DmaFromBuffer::from_ref(vaddr, paddr)
            })
    }
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
    fn read_blocks_uncached(
        &self,
        lba: u64,
        data_offset: u64,
        data_length: u64,
        rdata: &dyn DmaTarget,
    ) -> EResult<()>;

    /// Write uncached data blocks; bypasses the built-in page cache.
    fn write_blocks_uncached(
        &self,
        lba: u64,
        data_offset: u64,
        data_length: u64,
        rdata: &dyn DmaTarget,
    ) -> EResult<()>;
}

impl dyn BlockDevice {
    /// Get the volume information.
    /// If `force_probe` is `true`, probe for partitions even if they had been probed already.
    pub fn volume_info(&self, force_probe: bool) -> EResult<Option<VolumeInfo>> {
        if !force_probe {
            let guard = self.block_base().vinfo.unintr_lock_shared();
            if guard.vinfo_probed {
                return Ok(guard.vinfo.clone());
            }
        }

        let mut guard = self.block_base().vinfo.unintr_lock();
        if guard.vinfo_probed && !force_probe {
            return Ok(guard.vinfo.clone());
        }

        let info = get_volume_info(self)?;
        guard.vinfo = info.clone();
        guard.vinfo_probed = true;

        Ok(info)
    }

    /// Get or allocate the metadata and page cache.
    fn alloc_cache_mut<'a>(&'a self) -> EResult<MutexGuard<'a, BlockCaches>> {
        let base = self.block_base();

        let mut guard = base.cache.unintr_lock();
        if guard.is_none() {
            let ident = self.identify_uncached()?;
            let cache = PageCache::new(
                ident.block_size_exp,
                ident.block_count << ident.block_size_exp,
            );
            *guard = Some(BlockCaches { cache, ident });
        }

        Ok(guard.convert(|x| x.as_mut().unwrap()))
    }

    /// Get or allocate the metadata and page cache.
    fn alloc_cache<'a>(&'a self) -> EResult<SharedMutexGuard<'a, BlockCaches>> {
        let base = self.block_base();

        if let Some(x) = base.cache.unintr_lock_shared().try_convert(Option::as_ref) {
            return Ok(x);
        }

        let mut guard = base.cache.unintr_lock();
        if guard.is_none() {
            let ident = self.identify_uncached()?;
            let cache = PageCache::new(
                ident.block_size_exp,
                ident.block_count << ident.block_size_exp,
            );
            *guard = Some(BlockCaches { cache, ident });
        }

        Ok(guard.demote().convert(|x| x.as_ref().unwrap()))
    }

    /// Get the last cached block size exponent.
    /// Returns meaningless values if [`Self::identify`] hasn't run or there is no media.
    pub fn block_size_exp(&self) -> u8 {
        self.block_base()
            .cache
            .unintr_lock_shared()
            .as_ref()
            .map_or(0, |x| x.ident.block_size_exp)
    }

    /// Get the last cached block count.
    /// Returns meaningless values if [`Self::identify`] hasn't run or there is no media.
    pub fn block_count(&self) -> u64 {
        self.block_base()
            .cache
            .unintr_lock_shared()
            .as_ref()
            .map_or(0, |x| x.ident.block_count)
    }

    /// Get the last cached address width.
    /// Returns meaningless values if [`Self::identify`] hasn't run or there is no media.
    pub fn addr_width(&self) -> u8 {
        self.block_base()
            .cache
            .unintr_lock_shared()
            .as_ref()
            .map_or(0, |x| x.ident.addr_width)
    }

    /// Get block device information.
    pub fn identify(&self) -> EResult<BlockIdent> {
        self.alloc_cache().map(|x| x.ident)
    }

    /// Read bytes through the cache.
    #[inline(always)]
    pub fn readk_bytes(&self, addr: u64, rdata: &mut [u8]) -> EResult<()> {
        self.read_bytes(addr, UserSliceMut::new_kernel_mut(rdata))
    }

    /// Read bytes through the cache.
    pub fn read_bytes(&self, addr: u64, rdata: UserSliceMut<u8>) -> EResult<()> {
        let pager = PagerGlue(self);
        self.alloc_cache()?.cache.read_bytes(&pager, addr, rdata)
    }

    /// Write bytes through the cache.
    #[inline(always)]
    pub fn writek_bytes(&self, addr: u64, wdata: &[u8]) -> EResult<()> {
        self.write_bytes(addr, UserSlice::new_kernel(wdata))
    }

    /// Write bytes through the cache.
    pub fn write_bytes(&self, addr: u64, wdata: UserSlice<u8>) -> EResult<()> {
        let pager = PagerGlue(self);
        self.alloc_cache()?.cache.write_bytes(&pager, addr, wdata)
    }

    /// Write zeroes through the cache.
    pub fn write_zeroes(&self, addr: u64, len: u64) -> EResult<()> {
        let pager = PagerGlue(self);
        self.alloc_cache()?.cache.write_zeroes(&pager, addr, len)
    }

    /// Sync bytes from the cache to disk.
    /// If `flush` is `true`, removes cached reads as well.
    pub fn sync_bytes(&self, addr: u64, len: u64, flush: bool) -> EResult<()> {
        let pager = PagerGlue(self);
        let meta = self.alloc_cache()?;
        meta.cache.sync(&pager, addr, len)?;
        if flush {
            meta.cache.flush();
        }
        Ok(())
    }

    /// Sync all data from the cache to disk.
    /// If `flush` is `true`, removes cached reads as well.
    pub fn sync_all(&self, flush: bool) -> EResult<()> {
        let pager = PagerGlue(self);
        let meta = self.alloc_cache()?;
        meta.cache.sync_all(&pager)?;
        if flush {
            meta.cache.flush();
        }
        Ok(())
    }
}

/// Block device identification and metadata.
#[derive(Clone, Copy)]
pub struct BlockIdent {
    /// Log-base 2 of the block size.
    pub block_size_exp: u8,
    /// Total block count.
    pub block_count: u64,
    /// Maximum address width.
    pub addr_width: u8,
}
