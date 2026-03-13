// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering, fence};

use alloc::{boxed::Box, sync::Arc};

use crate::{
    badgelib::{self, irq::IrqGuard},
    bindings::{
        error::{EResult, Errno},
        raw::timestamp_us_t,
        spinlock::Spinlock,
    },
    cpu::irq,
    kernel::sync::waitlist::Waitlist,
    process::usercopy::{UserSlice, UserSliceMut},
};

use super::{File, SeekMode, Stat, VNode};

pub type FifoBuffer = badgelib::fifo::Fifo;

/// Data shared between all FIFO handles, regardless of whether it has a vnode.
pub(super) struct FifoShared {
    /// FIFO data storage.
    buffer: Spinlock<Option<Box<FifoBuffer>>>,
    /// Number of readers.
    read_count: AtomicU32,
    /// Number of writers.
    write_count: AtomicU32,
    /// Waiting list for readers.
    read_queue: Waitlist,
    /// Waiting list for writers.
    write_queue: Waitlist,
}

impl FifoShared {
    /// Create new shared FIFO data.
    pub(super) fn new() -> Arc<Self> {
        let tmp = Arc::new(Self {
            buffer: Spinlock::new(None),
            read_count: AtomicU32::new(0),
            write_count: AtomicU32::new(0),
            read_queue: Waitlist::new(),
            write_queue: Waitlist::new(),
        });
        fence(Ordering::Release);
        tmp
    }

    /// Handle a file open on a FIFO.
    pub(super) fn open(&self, nonblock: bool, is_read: bool, is_write: bool) -> EResult<()> {
        assert!(is_read || is_write);
        assert!(unsafe { irq::disable() });
        let nonblock = nonblock || (is_read && is_write);

        if is_read {
            self.read_count.fetch_add(1, Ordering::Relaxed);
        }
        if is_write {
            self.write_count.fetch_add(1, Ordering::Relaxed);
        }

        let mut guard = if nonblock {
            self.buffer.lock()
        } else {
            let queue = if is_read {
                &self.read_queue
            } else {
                &self.write_queue
            };
            let mut taken_lock = None;
            let taken_lock_ptr = &mut taken_lock;

            let res = queue.block(timestamp_us_t::MAX, move || {
                *taken_lock_ptr = Some(self.buffer.lock());

                // Unblock if the other end is open.
                if is_read && self.write_count.load(Ordering::Acquire) != 0 {
                    return true;
                } else if !is_read && self.read_count.load(Ordering::Acquire) != 0 {
                    return true;
                };

                *taken_lock_ptr = None;
                false
            });

            if let Err(x) = res {
                // Only happens if signalled.
                if is_read {
                    self.read_count.fetch_sub(1, Ordering::Relaxed);
                }
                if is_write {
                    self.write_count.fetch_sub(1, Ordering::Relaxed);
                }
                drop(taken_lock);
                unsafe { irq::enable() };
                return Err(x);
            }

            // Take the spinlock had it not been taken already.
            taken_lock.unwrap_or_else(|| self.buffer.lock())
        };

        // If both readers and writers exist, and there is no buffer, create it.
        if self.read_count.load(Ordering::Relaxed) != 0
            && self.write_count.load(Ordering::Relaxed) != 0
            && guard.is_none()
        {
            *guard = Some(Box::new(FifoBuffer::new(FifoBuffer::DEFAULT_SIZE).unwrap()));
        }
        drop(guard);

        // Wake writers on the fifo.
        self.write_queue.notify();

        unsafe { irq::enable() };

        Ok(())
    }

    /// Handle a file close on the FIFO.
    fn close(&self, had_read: bool, had_write: bool) {
        // TODO: Destroy buffer if all readers or all writers close.
        if had_read {
            self.read_count.fetch_sub(1, Ordering::Relaxed);
        }
        if had_write {
            self.write_count.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Handle a file read for a FIFO.
    /// WARNING: May sporadically return 0 in a blocking multi-read scenario.
    fn read(&self, nonblock: bool, rdata: UserSliceMut<'_, u8>) -> EResult<usize> {
        let _noirq = IrqGuard::new();

        let buffer = if nonblock {
            self.buffer.lock_shared()
        } else {
            let mut buffer = None;
            let buffer_ptr = &mut buffer;
            self.read_queue.block(timestamp_us_t::MAX, || {
                // Unblock if there is read data available.
                *buffer_ptr = Some(self.buffer.lock_shared());
                if let Some(buffer) = &**buffer_ptr.as_ref().unwrap()
                    && buffer.read_avl() > 0
                {
                    return false;
                }
                *buffer_ptr = None;
                true
            })?;
            buffer.unwrap_or_else(|| self.buffer.lock_shared())
        };

        let mut res = try { buffer.as_ref().ok_or(Errno::EAGAIN)?.read(rdata)? };
        if res == Ok(0) && nonblock {
            res = Err(Errno::EAGAIN)
        };

        // Wake blocking writers.
        self.write_queue.notify();

        res
    }

    /// Handle a file write for a FIFO.
    /// Raises EPIPE if `enforce_open` is true and the read end is closed.
    fn write(
        &self,
        nonblock: bool,
        wdata: UserSlice<'_, u8>,
        enforce_open: bool,
    ) -> EResult<usize> {
        let _noirq = IrqGuard::new();
        if enforce_open && self.read_count.load(Ordering::Relaxed) == 0 {
            return Err(Errno::EPIPE);
        }

        let buffer = if nonblock {
            self.buffer.lock_shared()
        } else {
            let mut buffer = None;
            let buffer_ptr = &mut buffer;
            self.write_queue.block(timestamp_us_t::MAX, || {
                // Unblock if there is write data available.
                *buffer_ptr = Some(self.buffer.lock_shared());
                if let Some(buffer) = &**buffer_ptr.as_ref().unwrap()
                    && buffer.write_avl() > 0
                {
                    return false;
                }
                *buffer_ptr = None;
                true
            })?;
            buffer.unwrap_or_else(|| self.buffer.lock_shared())
        };

        let mut res = try { buffer.as_ref().ok_or(Errno::EAGAIN)?.write(wdata)? };
        if res == Ok(0) && nonblock {
            res = Err(Errno::EAGAIN)
        };

        // Wake blocking readers.
        self.read_queue.notify();

        res
    }
}
unsafe impl Send for FifoShared {}
unsafe impl Sync for FifoShared {}

/// A FIFO or a pipe file descriptor.
pub struct Fifo {
    /// VNode, if any.
    pub(super) vnode: Option<Arc<VNode>>,
    /// Access is non-blocking.
    pub(super) is_nonblock: bool,
    /// This handle allows reading.
    pub(super) allow_read: bool,
    /// This handle allows writing.
    pub(super) allow_write: bool,
    /// Handle to the FIFO data buffer.
    pub(super) shared: Arc<FifoShared>,
}

impl Drop for Fifo {
    fn drop(&mut self) {
        self.shared.close(self.allow_read, self.allow_write);
    }
}

impl File for Fifo {
    fn stat(&self) -> EResult<Stat> {
        if let Some(vnode) = &self.vnode {
            vnode.mtx.lock_shared()?.ops.stat(&vnode)
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

    fn write(&self, mut wdata: UserSlice<'_, u8>) -> EResult<usize> {
        let mut wlen = self.shared.write(self.is_nonblock, wdata, false)?;
        wdata = wdata.subslice(wlen..wdata.len());
        while wdata.len() > 0 {
            match self.shared.write(self.is_nonblock, wdata, true) {
                Ok(res) => {
                    wlen += res;
                    wdata = wdata.subslice(res..wdata.len());
                }
                Err(Errno::EPIPE) => return Err(Errno::EPIPE),
                Err(_) => break,
            }
        }
        Ok(wlen)
    }

    fn read(&self, mut rdata: UserSliceMut<'_, u8>) -> EResult<usize> {
        let mut rlen = self.shared.read(self.is_nonblock, rdata)?;
        rdata = rdata.subslice_mut(rlen..rdata.len());
        while rdata.len() > 0
            && let Ok(res) = self.shared.read(true, rdata)
        {
            rlen += res;
            rdata = rdata.subslice_mut(res..rdata.len());
        }
        Ok(rlen)
    }

    fn resize(&self, _size: u64) -> EResult<()> {
        Err(Errno::ESPIPE)
    }

    fn sync(&self) -> EResult<()> {
        Err(Errno::ESPIPE)
    }

    fn get_vnode(&self) -> Option<Arc<VNode>> {
        self.vnode.clone()
    }
}
