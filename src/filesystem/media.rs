use alloc::sync::Arc;
use num::traits::{FromBytes, ToBytes};

use crate::{
    bindings::error::{EResult, Errno},
    device::class::block::BlockDevice,
    mem::dma::DmaTarget,
    process::usercopy::{UserSlice, UserSliceMut},
};

/// Specifies a partition to mount a filesystem on.
pub struct Media {
    /// Partition byte offset.
    pub offset: u64,
    /// Partition byte size.
    pub size: u64,
    /// Partition underlying storage.
    pub storage: Arc<dyn BlockDevice>,
}

impl Media {
    /// Write zeroes to the media.
    pub fn write_zeroes(&self, offset: u64, len: u64) -> EResult<()> {
        let offset = offset.checked_add(self.offset).ok_or(Errno::EIO)?;
        let end = offset.checked_add(len as u64).ok_or(Errno::EIO)?;
        if end > self.size {
            return Err(Errno::EIO);
        }
        self.storage.write_zeroes(offset, len)
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

        let block_size = 1 << self.storage.block_size_exp();
        if offset % block_size != 0 || end % block_size != 0 {
            return Err(Errno::EALIGN);
        }
        let block = offset / block_size;

        self.storage
            .write_blocks_uncached(block, data_offset, data_length, data)
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
        self.storage.write_bytes(offset, data)
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

        let block_size = 1 << self.storage.block_size_exp();
        if offset % block_size != 0 || end % block_size != 0 {
            return Err(Errno::EALIGN);
        }
        let block = offset / block_size;

        self.storage
            .read_blocks_uncached(block, data_offset, data_length, data)
    }

    /// Read data from the media.
    #[inline(always)]
    pub fn readk(&self, offset: u64, data: &mut [u8]) -> EResult<()> {
        self.read(offset, UserSliceMut::new_kernel_mut(data))
    }

    /// Read data from the media.
    pub fn read(&self, offset: u64, data: UserSliceMut<'_, u8>) -> EResult<()> {
        let offset = offset.checked_add(self.offset).ok_or(Errno::EIO)?;
        let end = offset.checked_add(data.len() as u64).ok_or(Errno::EIO)?;
        if end > self.size {
            return Err(Errno::EIO);
        }
        self.storage.read_bytes(offset, data)
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
        self.storage.sync_bytes(offset, len, false)
    }

    /// Device this media is attached to, if any.
    pub fn device(&self) -> Arc<dyn BlockDevice> {
        self.storage.clone()
    }
}
