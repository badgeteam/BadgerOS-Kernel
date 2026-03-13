use core::{cell::UnsafeCell, fmt::Debug};

use alloc::boxed::Box;
use num::traits::{FromBytes, ToBytes};

use crate::{
    bindings::{
        device::{HasBaseDevice, class::block::BlockDevice},
        error::{EResult, Errno},
    },
    mem::vmm::zeroes,
    process::usercopy::{UserSlice, UserSliceMut},
};

/// Specifies some type of media a filesystem can be mounted on.
pub enum MediaType {
    Block(BlockDevice),
    Ram(UnsafeCell<Box<[u8]>>),
}

impl Debug for MediaType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Block(arg0) => f.debug_tuple("Block").field(&arg0.id()).finish(),
            Self::Ram(arg0) => f
                .debug_tuple("Ram")
                .field(&unsafe { arg0.as_ref_unchecked() }.len())
                .finish(),
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
                let zeroes = zeroes();
                let end = offset + len;
                let mut offset = offset;
                while offset < end {
                    let max = (end - offset).min(zeroes.len() as u64) as usize;
                    block_device.writek_bytes(offset, &zeroes[..max])?;
                    offset += max as u64;
                }
            }
            MediaType::Ram(ram) => {
                let buffer = unsafe { ram.as_mut_unchecked() };
                buffer[offset as usize..offset as usize + len as usize].fill(0);
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
    pub fn device(&self) -> Option<BlockDevice> {
        match &self.storage {
            MediaType::Block(block_device) => Some(block_device.clone()),
            _ => None,
        }
    }
}
