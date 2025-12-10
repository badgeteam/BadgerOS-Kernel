// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::{boxed::Box, vec::Vec};

use crate::{
    bindings::error::EResult,
    process::usercopy::{AccessResult, UserSlice, UserSliceMut},
};

/// FIFO data buffer.
pub struct Fifo {
    /// Data buffer.
    data: UnsafeCell<Box<[u8]>>,
    /// Read reserved position.
    read_resv: AtomicUsize,
    /// Read commit position.
    read_commit: AtomicUsize,
    /// Write reserved position.
    write_resv: AtomicUsize,
    /// Write commit position.
    write_commit: AtomicUsize,
}
unsafe impl Sync for Fifo {}

impl Fifo {
    pub const DEFAULT_SIZE: usize = 2048;

    /// Create a new FIFO data buffer.
    pub fn new(size: usize) -> EResult<Self> {
        let mut data = Vec::new();
        data.try_reserve_exact(size)?;
        data.resize(size, 0);
        Ok(Self {
            data: UnsafeCell::new(data.into_boxed_slice()),
            read_resv: AtomicUsize::new(0),
            read_commit: AtomicUsize::new(0),
            write_resv: AtomicUsize::new(0),
            write_commit: AtomicUsize::new(0),
        })
    }

    /// Read data from the buffer.
    #[inline(always)]
    pub fn readk(&self, rdata: &mut [u8]) -> usize {
        unsafe {
            self.read(UserSliceMut::new_kernel_mut(rdata))
                .unwrap_unchecked()
        }
    }

    /// Read data from the buffer.
    pub fn read(&self, mut rdata: UserSliceMut<'_, u8>) -> AccessResult<usize> {
        let ptr = unsafe { self.data.as_ref_unchecked() };
        let cap = ptr.len();

        // Try to reserve data.
        let mut rx = self.read_resv.load(Ordering::Relaxed);
        let tx = self.write_commit.load(Ordering::Relaxed);
        let mut recv_cap;

        loop {
            recv_cap = (tx.wrapping_sub(rx).wrapping_add(cap) % cap).min(rdata.len());
            if let Err(x) = self.read_resv.compare_exchange(
                rx,
                rx.wrapping_add(recv_cap) % cap,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                rx = x;
            } else {
                break;
            }
        }
        if recv_cap == 0 {
            return Ok(0);
        }

        // Copy data out of the FIFO's buffer.
        let start_off = rx;
        let end_off = rx.wrapping_add(recv_cap) % cap;
        let res: AccessResult<()> = try {
            if end_off > start_off {
                rdata.write_multiple(0, &ptr[start_off..end_off])?;
            } else {
                rdata.write_multiple(0, &ptr[start_off..cap])?;
                rdata.write_multiple(cap - start_off, &ptr[..end_off])?;
            }
        };

        // Mark the read as completed.
        while let Err(_) = self.read_commit.compare_exchange(
            start_off,
            end_off,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {}
        // Make sure to propagate the EFAULT that could be raised.
        res?;

        Ok(recv_cap)
    }

    /// Read data from the buffer.
    #[inline(always)]
    pub fn writek(&self, rdata: &[u8]) -> usize {
        unsafe { self.write(UserSlice::new_kernel(rdata)).unwrap_unchecked() }
    }

    /// Write data to the buffer.
    pub fn write(&self, wdata: UserSlice<'_, u8>) -> AccessResult<usize> {
        let ptr = unsafe { self.data.as_mut_unchecked() };
        let cap = ptr.len();

        // Try to reserve space.
        let rx = self.read_commit.load(Ordering::Relaxed);
        let mut tx = self.write_resv.load(Ordering::Relaxed);
        let mut send_cap: usize;

        loop {
            send_cap =
                (rx.wrapping_sub(rx).wrapping_add(cap).wrapping_sub(1) % cap).min(wdata.len());
            if let Err(x) = self.write_resv.compare_exchange(
                tx,
                tx.wrapping_add(send_cap) % cap,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                tx = x;
            } else {
                break;
            }
        }
        if send_cap == 0 {
            return Ok(0);
        }

        // Copy the data into the FIFO's buffer.
        let start_off = tx;
        let end_off = start_off.wrapping_add(send_cap) % cap;
        let res: AccessResult<()> = try {
            if end_off > start_off {
                wdata.read_multiple(0, &mut ptr[start_off..end_off])?;
            } else {
                wdata.read_multiple(0, &mut ptr[start_off..cap])?;
                wdata.read_multiple(cap - start_off, &mut ptr[..end_off])?;
            }
        };

        // Mark the write as completed.
        while let Err(_) = self.write_commit.compare_exchange(
            start_off,
            end_off,
            Ordering::Release,
            Ordering::Relaxed,
        ) {}
        // Make sure to propagate the EFAULT that could be raised.
        res?;

        Ok(send_cap)
    }

    /// Get the amount of available read data.
    pub fn read_avl(&self) -> usize {
        let cap = unsafe { self.data.as_ref_unchecked() }.len();
        let rx = self.read_resv.load(Ordering::Relaxed);
        let tx = self.write_commit.load(Ordering::Relaxed);
        tx.wrapping_sub(rx).wrapping_add(cap) % cap
    }

    /// Get the amount of available write space.
    pub fn write_avl(&self) -> usize {
        let cap = unsafe { self.data.as_ref_unchecked() }.len();
        let rx = self.read_resv.load(Ordering::Relaxed);
        let tx = self.write_commit.load(Ordering::Relaxed);
        rx.wrapping_sub(tx).wrapping_add(cap).wrapping_sub(1) % cap
    }
}
