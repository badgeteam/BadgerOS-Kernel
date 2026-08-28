// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{fmt::Display, ops::Range, ptr::NonNull};

use alloc::{sync::Arc, vec::Vec};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    device::{Device, DeviceBase, registry},
    device_get_trait_vtable,
    filesystem::partition::{VolumeInfo, get_volume_info},
    kcore::sync::mutex::{Mutex, MutexGuard, SharedMutexGuard},
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
    part_devs: Vec<Arc<BlockDevicePart>>,
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
                part_devs: Vec::new(),
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

    /// Get the partition translation, if any.
    /// Should return [`None`] for regular block devices.
    fn current_partition(&self) -> Option<Range<u64>> {
        None
    }

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
    pub fn volume_info(self: &Arc<Self>, force_probe: bool) -> EResult<Option<VolumeInfo>> {
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

        let info = get_volume_info(&**self)?;
        guard.vinfo = info.clone();
        guard.vinfo_probed = true;

        // Update partition devices.
        let base = self.base();
        if let Some(name) = base.node_name()
            && let Some(number) = base.node_num()
        {
            // Remove existing partitions that no longer exist.
            for dev in guard.part_devs.drain(0..) {
                registry::remove_device(&*dev);
            }

            if let Some(info) = &info {
                // Create/replace partitions that are new/updated.
                for i in 0..info.parts.len() {
                    let part = &info.parts[i];
                    let part = Arc::new(BlockDevicePart {
                        base: DeviceBase::with_node_name(format!("{}{}p", name, number), false),
                        parent: self.clone(),
                        index: i as u32,
                        range: part.offset..part.offset + part.size,
                    });
                    match registry::register_device(part.clone()) {
                        Ok(_) => guard.part_devs.push(part),
                        Err(x) => logkf!(
                            LogLevel::Error,
                            "Failed to register partition {} for {}{}: {}",
                            i,
                            name,
                            number,
                            x
                        ),
                    }
                }
            }
        }

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

    /// Get the length of the current partition or, if none, the entire block device.
    /// Returns meaningless values if [`Self::identify`] hasn't run or there is no media.
    pub fn len(&self) -> u64 {
        if let Some(range) = self.current_partition() {
            range.end - range.start
        } else {
            self.block_base()
                .cache
                .unintr_lock_shared()
                .as_ref()
                .map_or(0, |x| x.ident.block_count << x.ident.block_size_exp)
        }
    }

    /// Partition offset and bounds-checking helper.
    fn partition_offset(&self, addr: u64, len: u64) -> EResult<u64> {
        if let Some(partition) = self.current_partition() {
            let start = addr.checked_add(partition.start).ok_or(Errno::EIO)?;
            if start.checked_add(len).ok_or(Errno::EIO)? > partition.end {
                return Err(Errno::EIO);
            }
            Ok(start)
        } else {
            Ok(addr)
        }
    }

    /// Read bytes through the cache.
    #[inline(always)]
    pub fn readk_bytes(&self, addr: u64, rdata: &mut [u8]) -> EResult<()> {
        self.read_bytes(addr, UserSliceMut::new_kernel_mut(rdata))
    }

    /// Read bytes through the cache.
    pub fn read_bytes(&self, addr: u64, rdata: UserSliceMut<u8>) -> EResult<()> {
        let addr = self.partition_offset(addr, rdata.len() as u64)?;
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
        let addr = self.partition_offset(addr, wdata.len() as u64)?;
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
    pub fn sync_bytes(&self, mut addr: u64, mut len: u64, flush: bool) -> EResult<()> {
        if let Some(partition) = self.current_partition() {
            addr = addr.checked_add(partition.start).ok_or(Errno::EIO)?;
            // Silently clamp the sync range instead of outright rejecting it.
            len = len.min(partition.end - addr);
        }
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
        if let Some(partition) = self.current_partition() {
            meta.cache
                .sync(&pager, partition.start, partition.end - partition.start)?;
        } else {
            meta.cache.sync_all(&pager)?;
        }
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

/// Implementation of [`BlockDevice`] for partitions on another.
pub struct BlockDevicePart {
    base: DeviceBase,
    parent: Arc<dyn BlockDevice>,
    index: u32,
    range: Range<u64>,
}

impl Display for BlockDevicePart {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} partition {}", self.parent, self.index)
    }
}

impl Device for BlockDevicePart {
    fn base(&self) -> &DeviceBase {
        &self.base
    }

    fn interrupt(&self, _id: u128) -> bool {
        unreachable!()
    }

    device_get_trait_vtable!(BlockDevice);
}

impl BlockDevice for BlockDevicePart {
    fn block_base(&self) -> &BlockDeviceBase {
        self.parent.block_base()
    }

    fn current_partition(&self) -> Option<Range<u64>> {
        Some(self.range.clone())
    }

    fn identify_uncached(&self) -> EResult<BlockIdent> {
        logkf!(
            LogLevel::Warning,
            "Attempt to call identify_uncached on BlockDevicePart"
        );
        Err(Errno::EINVAL)
    }

    fn read_blocks_uncached(
        &self,
        lba: u64,
        data_offset: u64,
        data_length: u64,
        rdata: &dyn DmaTarget,
    ) -> EResult<()> {
        self.parent
            .read_blocks_uncached(lba, data_offset, data_length, rdata)
    }

    fn write_blocks_uncached(
        &self,
        lba: u64,
        data_offset: u64,
        data_length: u64,
        rdata: &dyn DmaTarget,
    ) -> EResult<()> {
        self.parent
            .write_blocks_uncached(lba, data_offset, data_length, rdata)
    }
}
