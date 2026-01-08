use core::sync::atomic::Ordering;

use crate::bindings::{
    device::{HasBaseDevice, class::char::CharDevice},
    error::EResult,
};

use super::*;

/// A character device bound to a VNode.
pub(super) struct CharDevFile {
    /// The character device associated with this file.
    char_dev: CharDevice,
    /// The VNode at which this device is bound.
    vnode: Arc<VNode>,
}

impl CharDevFile {
    /// Create a new character device file.
    pub fn new(vnode: Arc<VNode>) -> Self {
        Self {
            char_dev: vnode
                .clone()
                .mtx
                .unintr_lock_shared()
                .ops
                .get_device(&vnode)
                .unwrap()
                .as_char()
                .unwrap()
                .clone(),
            vnode,
        }
    }
}

impl File for CharDevFile {
    fn stat(&self) -> EResult<Stat> {
        self.vnode.mtx.lock_shared()?.ops.stat(&self.vnode)
    }

    fn tell(&self) -> EResult<u64> {
        Err(Errno::ESPIPE)
    }

    fn seek(&self, _mode: SeekMode, _offset: i64) -> EResult<u64> {
        Err(Errno::ESPIPE)
    }

    fn write(&self, wdata: UserSlice<'_, u8>) -> EResult<usize> {
        self.char_dev.write(wdata)
    }

    fn read(&self, rdata: UserSliceMut<'_, u8>) -> EResult<usize> {
        self.char_dev.read(rdata)
    }

    fn resize(&self, _size: u64) -> EResult<()> {
        Err(Errno::ESPIPE)
    }

    fn sync(&self) -> EResult<()> {
        Ok(())
    }

    fn get_vnode(&self) -> Option<Arc<VNode>> {
        Some(self.vnode.clone())
    }

    fn get_device(&self) -> Option<BaseDevice> {
        Some(self.char_dev.as_base().clone())
    }
}

/// A block device bound to a VNode.
pub(super) struct BlockDevFile {
    /// The block device associated with this file.
    block_dev: BlockDevice,
    /// The VNode at which this device is bound.
    vnode: Arc<VNode>,
    /// The access offset for this file.
    offset: AtomicU64,
    /// This handle allows reading.
    allow_read: bool,
    /// This handle allows writing.
    allow_write: bool,
}

impl BlockDevFile {
    /// Create a new block device file.
    pub fn new(vnode: Arc<VNode>, allow_read: bool, allow_write: bool) -> Self {
        Self {
            block_dev: vnode
                .clone()
                .mtx
                .unintr_lock_shared()
                .ops
                .get_device(&vnode)
                .unwrap()
                .as_block()
                .unwrap()
                .clone(),
            vnode,
            offset: AtomicU64::new(0),
            allow_read,
            allow_write,
        }
    }
}

impl File for BlockDevFile {
    fn stat(&self) -> EResult<Stat> {
        self.vnode.mtx.lock_shared()?.ops.stat(&self.vnode)
    }

    fn tell(&self) -> EResult<u64> {
        Ok(self.offset.load(Ordering::Relaxed))
    }

    fn seek(&self, mode: SeekMode, offset: i64) -> EResult<u64> {
        let size = self.block_dev.block_count() << self.block_dev.block_size_exp();
        let mut old_off = self.offset.load(Ordering::Relaxed);

        let mut new_off = match mode {
            SeekMode::Set => offset.clamp(0, size as i64),
            SeekMode::Cur => offset.saturating_add(old_off as i64).clamp(0, size as i64),
            SeekMode::End => offset.saturating_add(size as i64).clamp(0, size as i64),
        } as u64;

        while let Err(x) =
            self.offset
                .compare_exchange(old_off, new_off, Ordering::Relaxed, Ordering::Relaxed)
        {
            old_off = x;
            new_off = match mode {
                SeekMode::Set => offset.clamp(0, size as i64),
                SeekMode::Cur => offset.saturating_add(old_off as i64).clamp(0, size as i64),
                SeekMode::End => offset.saturating_add(size as i64).clamp(0, size as i64),
            } as u64;
        }

        Ok(new_off)
    }

    fn write(&self, wdata: UserSlice<'_, u8>) -> EResult<usize> {
        if !self.allow_write {
            return Err(Errno::EBADF);
        }

        let size = self.block_dev.block_count() << self.block_dev.block_size_exp();

        // Increment offset and determine read count.
        let mut offset = self.offset.load(Ordering::Acquire);
        let mut readlen = (wdata.len() as u64).min(size.saturating_sub(offset)) as usize;
        while let Err(x) = self.offset.compare_exchange(
            offset,
            offset + readlen as u64,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            offset = x;
            readlen = (wdata.len() as u64).min(size.saturating_sub(offset)) as usize;
        }

        // Perform read on device.
        self.block_dev.write_bytes(offset, wdata)?;
        Ok(readlen)
    }

    fn read(&self, rdata: UserSliceMut<'_, u8>) -> EResult<usize> {
        if !self.allow_read {
            return Err(Errno::EBADF);
        }

        let size = self.block_dev.block_count() << self.block_dev.block_size_exp();

        // Increment offset and determine read count.
        let mut offset = self.offset.load(Ordering::Acquire);
        let mut readlen = (rdata.len() as u64).min(size.saturating_sub(offset)) as usize;
        while let Err(x) = self.offset.compare_exchange(
            offset,
            offset + readlen as u64,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            offset = x;
            readlen = (rdata.len() as u64).min(size.saturating_sub(offset)) as usize;
        }

        // Perform read on device.
        self.block_dev.read_bytes(offset, rdata)?;
        Ok(readlen)
    }

    fn resize(&self, _size: u64) -> EResult<()> {
        Err(Errno::ENOSYS)
    }

    fn sync(&self) -> EResult<()> {
        self.block_dev.sync_all(false)
    }

    fn get_vnode(&self) -> Option<Arc<VNode>> {
        Some(self.vnode.clone())
    }

    fn get_device(&self) -> Option<BaseDevice> {
        Some(self.block_dev.as_base().clone())
    }
}
