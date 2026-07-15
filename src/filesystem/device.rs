use crate::{
    bindings::error::EResult,
    dev2::{Device, class::char::CharDevice},
};

use super::*;

/// A character device bound to a VNode.
pub struct CharDevFile {
    char_dev: Arc<dyn CharDevice>,
    loc: Option<VfsLoc>,
    flags: Mutex<u32>,
}

impl CharDevFile {
    /// Create a new character device file from a VNode.
    pub fn new(loc: Option<VfsLoc>, char_dev: Arc<dyn CharDevice>, flags: u32) -> Self {
        Self {
            char_dev,
            loc,
            flags: Mutex::new(flags),
        }
    }
}

impl File for CharDevFile {
    fn poll(&self) -> u32 {
        self.char_dev.poll()
    }

    fn poll_waitlists<'a>(&'a self, interest: u32, collect: &mut Vec<&'a Waitlist>) -> EResult<()> {
        self.char_dev.poll_waitlists(interest, collect)
    }

    fn get_flags(&self) -> u32 {
        *self.flags.unintr_lock_shared()
    }

    fn set_flags(&self, newfl: u32) -> EResult<()> {
        *self.flags.lock()? = newfl;
        Ok(())
    }

    fn isatty(&self) -> EResult<()> {
        if self.char_dev.is_tty() {
            Ok(())
        } else {
            Err(Errno::ENOTTY)
        }
    }

    fn get_dirents(&self, _buffer: &mut DentBuffer<'_>) -> EResult<()> {
        Err(Errno::ENOTDIR)
    }

    fn stat(&self) -> EResult<Stat> {
        if let Some(ref loc) = self.loc {
            loc.vnode.mtx.lock_shared()?.ops.stat(&loc.vnode)
        } else {
            Ok(Stat::default())
        }
    }

    fn tell(&self) -> EResult<u64> {
        Err(Errno::ESPIPE)
    }

    fn seek(&self, _mode: SeekMode, _offset: i64) -> EResult<u64> {
        Err(Errno::ESPIPE)
    }

    fn write(&self, wdata: UserSlice<'_, u8>) -> EResult<usize> {
        let flags = *self.flags.unintr_lock_shared();
        if flags & oflags::WRITE_ONLY == 0 {
            return Err(Errno::EBADF);
        }
        self.char_dev
            .write_raw(wdata, flags & oflags::NONBLOCK != 0)
    }

    fn read(&self, rdata: UserSliceMut<'_, u8>) -> EResult<usize> {
        let flags = *self.flags.unintr_lock_shared();
        if flags & oflags::READ_ONLY == 0 {
            return Err(Errno::EBADF);
        }
        self.char_dev.read_raw(rdata, flags & oflags::NONBLOCK != 0)
    }

    fn resize(&self, _size: u64) -> EResult<()> {
        Err(Errno::ESPIPE)
    }

    fn sync(&self) -> EResult<()> {
        Ok(())
    }

    fn get_loc(&self) -> Option<VfsLoc> {
        self.loc.clone()
    }

    fn get_device(&self) -> Option<Arc<dyn Device>> {
        Some(self.char_dev.clone())
    }
}

/// A block device bound to a VNode.
pub(super) struct BlockDevFile {
    block_dev: Arc<dyn BlockDevice>,
    loc: VfsLoc,
    flags: Mutex<FlagsAndOffset>,
}

impl BlockDevFile {
    /// Create a new block device file.
    pub fn new(loc: VfsLoc, flags: u32) -> Self {
        Self {
            block_dev: loc
                .vnode
                .clone()
                .mtx
                .unintr_lock_shared()
                .ops
                .get_device(&loc.vnode)
                .unwrap()
                .try_as_arc::<dyn BlockDevice>()
                .unwrap()
                .clone(),
            loc,
            flags: Mutex::new(FlagsAndOffset { offset: 0, flags }),
        }
    }
}

impl File for BlockDevFile {
    fn poll(&self) -> u32 {
        // I/O is always non-blocking on raw block devices.
        poll::IN | poll::OUT
    }

    fn poll_waitlists<'a>(
        &'a self,
        _interest: u32,
        _collect: &mut Vec<&'a Waitlist>,
    ) -> EResult<()> {
        Ok(())
    }

    fn get_flags(&self) -> u32 {
        self.flags.unintr_lock_shared().flags
    }

    fn set_flags(&self, newfl: u32) -> EResult<()> {
        self.flags.lock()?.flags = newfl;
        Ok(())
    }

    fn get_dirents(&self, _buffer: &mut DentBuffer<'_>) -> EResult<()> {
        Err(Errno::ENOTDIR)
    }

    fn stat(&self) -> EResult<Stat> {
        self.loc.vnode.mtx.lock_shared()?.ops.stat(&self.loc.vnode)
    }

    fn tell(&self) -> EResult<u64> {
        Ok(self.flags.unintr_lock_shared().offset)
    }

    fn seek(&self, mode: SeekMode, offset: i64) -> EResult<u64> {
        let mut flags = self.flags.lock()?;
        let size = self.block_dev.block_count() << self.block_dev.block_size_exp();
        let old_off = flags.offset;

        let new_off = match mode {
            SeekMode::Set => offset.clamp(0, size as i64),
            SeekMode::Cur => offset.saturating_add(old_off as i64).clamp(0, size as i64),
            SeekMode::End => offset.saturating_add(size as i64).clamp(0, size as i64),
        } as u64;
        flags.offset = new_off;

        Ok(new_off)
    }

    fn write(&self, wdata: UserSlice<'_, u8>) -> EResult<usize> {
        let mut flags = self.flags.lock()?;
        if flags.flags & oflags::WRITE_ONLY == 0 {
            return Err(Errno::EBADF);
        }

        let size = self.block_dev.block_count() << self.block_dev.block_size_exp();

        // Increment offset and determine read count.
        let offset = flags.offset;
        let readlen = (wdata.len() as u64).min(size.saturating_sub(offset)) as usize;
        flags.offset = offset + readlen as u64;

        // Perform read on device.
        self.block_dev.write_bytes(offset, wdata)?;
        Ok(readlen)
    }

    fn read(&self, rdata: UserSliceMut<'_, u8>) -> EResult<usize> {
        let mut flags = self.flags.lock()?;
        if flags.flags & oflags::READ_ONLY == 0 {
            return Err(Errno::EBADF);
        }

        let size = self.block_dev.block_count() << self.block_dev.block_size_exp();

        // Increment offset and determine write count.
        let offset = flags.offset;
        let readlen = (rdata.len() as u64).min(size.saturating_sub(offset)) as usize;
        flags.offset = offset + readlen as u64;

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

    fn get_loc(&self) -> Option<VfsLoc> {
        Some(self.loc.clone())
    }

    fn get_device(&self) -> Option<Arc<dyn Device>> {
        Some(self.block_dev.clone())
    }
}
