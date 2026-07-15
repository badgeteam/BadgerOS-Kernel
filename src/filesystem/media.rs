use core::{cell::UnsafeCell, fmt::Debug};

use alloc::{boxed::Box, sync::Arc};
use num::traits::{FromBytes, ToBytes};

use crate::{
    bindings::error::{EResult, Errno},
    dev2::class::block::BlockDevice,
    mem::dma::{self, DmaTarget},
    process::usercopy::{UserSlice, UserSliceMut},
};

/// Specifies some type of media a filesystem can be mounted on.
pub enum MediaType {
    Block(Arc<dyn BlockDevice>),
    Ram(UnsafeCell<Box<[u8]>>),
}

impl Debug for MediaType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Block(arg0) => arg0.fmt(f),
            Self::Ram(arg0) => f.write_fmt(format_args!("Ramdisk 0x{:x}", arg0.get() as usize)),
        }
    }
}

/// Specifies a partition to mount a filesystem on.
#[derive(Debug)]
pub struct Media {
    /// Partition byte offset.
    pub offset: u64,
    /// Partition byte size.
    pub size: u64,
    /// Partition underlying storage.
    // TODO: Make a RamDisk so that storage is just a BlockDevice handle.
    pub storage: MediaType,
}
unsafe impl Sync for Media {}

impl Media {
    /// Write zeroes to the media.
    pub fn write_zeroes(&self, offset: u64, len: u64) -> EResult<()> {
        let offset = offset.checked_add(self.offset).ok_or(Errno::EIO)?;
        let end = offset.checked_add(len as u64).ok_or(Errno::EIO)?;
        if end > self.size {
            return Err(Errno::EIO);
        }
        match &self.storage {
            MediaType::Block(block_device) => {
                block_device.write_zeroes(offset, len)?;
            }
            MediaType::Ram(ram) => {
                let buffer = unsafe { ram.as_mut_unchecked() };
                buffer[offset as usize..offset as usize + len as usize].fill(0);
            }
        }
        Ok(())
    }

    /// Use DMA to write data, bypassing the caches.
    /// Fails if the access is not aligned to disk blocks.
    pub fn write_uncached(
        &self,
        offset: u64,
        data_offset: u64,
        data_length: u64,
        data: &dyn DmaTarget,
    ) -> EResult<()> {
        let offset = offset.checked_add(self.offset).ok_or(Errno::EIO)?;
        let end = offset.checked_add(data.size() as u64).ok_or(Errno::EIO)?;
        if end > self.size {
            return Err(Errno::EIO);
        }

        match &self.storage {
            MediaType::Block(block_device) => {
                let block_size = 1 << block_device.block_size_exp();
                if offset % block_size != 0 || end % block_size != 0 {
                    return Err(Errno::EALIGN);
                }
                let block = offset / block_size;

                block_device.write_blocks_uncached(block, data_offset, data_length, data)?;
            }
            MediaType::Ram(ram) => {
                let buffer = unsafe { ram.as_mut_unchecked() };
                dma::cpu_gather(
                    data_offset,
                    data_length as usize,
                    data,
                    &mut buffer[offset as usize..end as usize],
                )?;
            }
        }

        Ok(())
    }

    /// Write data to the media.
    #[inline(always)]
    pub fn writek(&self, offset: u64, data: &[u8]) -> EResult<()> {
        self.write(offset, UserSlice::new_kernel(data))
    }

    /// Write data to the media.
    pub fn write(&self, offset: u64, data: UserSlice<'_, u8>) -> EResult<()> {
        let offset = offset.checked_add(self.offset).ok_or(Errno::EIO)?;
        let end = offset.checked_add(data.len() as u64).ok_or(Errno::EIO)?;
        if end > self.size {
            return Err(Errno::EIO);
        }
        match &self.storage {
            MediaType::Block(block_device) => {
                block_device.write_bytes(offset, data)?;
            }
            MediaType::Ram(ram) => {
                let buffer = unsafe { ram.as_mut_unchecked() };
                data.read_multiple(
                    0,
                    &mut buffer[offset as usize..offset as usize + data.len()],
                )?;
            }
        }
        Ok(())
    }

    /// Use DMA to read data, bypassing the caches.
    /// Fails if the access is not aligned to disk blocks.
    pub fn read_uncached(
        &self,
        offset: u64,
        data_offset: u64,
        data_length: u64,
        data: &dyn DmaTarget,
    ) -> EResult<()> {
        let offset = offset.checked_add(self.offset).ok_or(Errno::EIO)?;
        let end = offset.checked_add(data.size() as u64).ok_or(Errno::EIO)?;
        if end > self.size {
            return Err(Errno::EIO);
        }

        match &self.storage {
            MediaType::Block(block_device) => {
                let block_size = 1 << block_device.block_size_exp();
                if offset % block_size != 0 || end % block_size != 0 {
                    return Err(Errno::EALIGN);
                }
                let block = offset / block_size;

                block_device.read_blocks_uncached(block, data_offset, data_length, data)?;
            }
            MediaType::Ram(ram) => {
                let buffer = unsafe { ram.as_ref_unchecked() };
                dma::cpu_scatter(
                    data_offset,
                    data_length as usize,
                    data,
                    &buffer[offset as usize..end as usize],
                )?;
            }
        }

        Ok(())
    }

    /// Read data from the media.
    #[inline(always)]
    pub fn readk(&self, offset: u64, data: &mut [u8]) -> EResult<()> {
        self.read(offset, UserSliceMut::new_kernel_mut(data))
    }

    /// Read data from the media.
    pub fn read(&self, offset: u64, mut data: UserSliceMut<'_, u8>) -> EResult<()> {
        let offset = offset.checked_add(self.offset).ok_or(Errno::EIO)?;
        let end = offset.checked_add(data.len() as u64).ok_or(Errno::EIO)?;
        if end > self.size {
            return Err(Errno::EIO);
        }
        match &self.storage {
            MediaType::Block(block_device) => {
                block_device.read_bytes(offset, data)?;
            }
            MediaType::Ram(ram) => {
                let buffer = unsafe { ram.as_mut_unchecked() };
                data.write_multiple(0, &buffer[offset as usize..offset as usize + data.len()])?;
            }
        }
        Ok(())
    }

    /// Write little-endian bytes.
    pub fn write_le<T: ToBytes>(&self, offset: u64, data: T) -> EResult<()> {
        self.writek(offset, data.to_le_bytes().as_ref())
    }

    /// Read little-endian bytes.
    pub fn read_le<T: FromBytes>(&self, offset: u64) -> EResult<T>
    where
        T: FromBytes<Bytes = [u8; size_of::<T>()]>,
    {
        let mut tmp = [0u8; _];
        self.readk(offset, &mut tmp)?;
        Ok(T::from_le_bytes(&tmp))
    }

    /// Write big-endian bytes.
    pub fn write_be<T: ToBytes>(&self, offset: u64, data: T) -> EResult<()> {
        self.writek(offset, data.to_be_bytes().as_ref())
    }

    /// Read big-endian bytes.
    pub fn read_be<T: FromBytes>(&self, offset: u64) -> EResult<T>
    where
        T: FromBytes<Bytes = [u8; size_of::<T>()]>,
    {
        let mut tmp = [0u8; _];
        self.readk(offset, &mut tmp)?;
        Ok(T::from_be_bytes(&tmp))
    }

    /// Sync a region of the media.
    pub fn sync(&self, offset: u64, len: u64) -> EResult<()> {
        let offset = offset.checked_add(self.offset).ok_or(Errno::EIO)?;
        let end = offset.checked_add(len).ok_or(Errno::EIO)?;
        if end > self.size {
            return Err(Errno::EIO);
        }
        match &self.storage {
            MediaType::Block(block_device) => {
                block_device.sync_bytes(offset, len, false)?;
            }
            MediaType::Ram(_) => {
                // RAM doesn't need explicit sync.
            }
        }
        Ok(())
    }

    /// Device this media is attached to, if any.
    pub fn device(&self) -> Option<Arc<dyn BlockDevice>> {
        match &self.storage {
            MediaType::Block(block_device) => Some(block_device.clone()),
            _ => None,
        }
    }
}
