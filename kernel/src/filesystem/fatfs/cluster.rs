// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

//! Code that helps with allocation of clusters in memory only.

use core::{
    fmt::{Debug, Formatter},
    ops::Range,
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
};

use alloc::{collections::TryReserveError, vec::Vec};

use crate::{
    LogLevel,
    bindings::error::{EResult, Errno},
    process::usercopy::{UserSlice, UserSliceMut},
};

use super::FatFs;

/// Allocates clusters from a FAT.
pub(super) struct ClusterAlloc {
    /// Total number of clusters.
    total: u32,
    /// Amount of available clusters.
    available: AtomicU32,
    /// Bitmap of available clusters.
    bitmap: Vec<AtomicUsize>,
}

impl ClusterAlloc {
    /// Create a new allocator with no available clusters.
    pub(super) fn new(cluster_count: u32) -> Result<Self, TryReserveError> {
        let vec_len = ((cluster_count - 1) / usize::BITS + 1) as usize;
        let mut bitmap = Vec::try_with_capacity(vec_len)?;
        bitmap.resize_with(vec_len, || AtomicUsize::new(0));
        Ok(Self {
            total: cluster_count,
            available: AtomicU32::new(0),
            bitmap,
        })
    }

    /// Mark `cluster` as usable.
    pub(super) fn free(&self, cluster: u32) {
        let mask = 1usize << (cluster % usize::BITS);
        let prev = self.bitmap[(cluster / usize::BITS) as usize].fetch_or(mask, Ordering::Relaxed);
        if prev & mask != 0 {
            #[cfg(debug_assertions)]
            logkf!(
                LogLevel::Warning,
                "Cluster {} marked as usable twice",
                cluster
            );
        } else {
            self.available.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Mark all clusters from `chain` as usable.
    pub(super) fn free_chain(&self, chain: &ClusterChain) {
        for cluster in chain {
            self.free(cluster);
        }
    }

    /// Try to allocate a single cluster.
    pub(super) fn alloc(&self) -> EResult<u32> {
        let mut available = self.available.load(Ordering::Relaxed);
        while let Err(x) = self.available.compare_exchange_weak(
            available,
            available.checked_sub(1).ok_or(Errno::ENOSPC)?,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            available = x;
        }

        let mut i = 0;
        loop {
            'l0: loop {
                // Pop a single cluster.
                let mut cur = self.bitmap[i].load(Ordering::Relaxed);
                let mut mask = cur ^ (cur & cur.wrapping_sub(1));
                if mask == 0 {
                    break 'l0;
                }
                while let Err(x) = self.bitmap[i].compare_exchange(
                    cur,
                    cur & !mask,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    cur = x;
                    mask = cur ^ (cur & cur.wrapping_sub(1));
                    if mask == 0 {
                        break 'l0;
                    }
                }

                return Ok(mask.trailing_zeros() + i as u32 * usize::BITS);
            }
            i = (i + 1) % self.bitmap.len();
        }
    }

    /// Try to allocate a chain of clusters.
    pub(super) fn alloc_chain(&self, amount: u32) -> EResult<ClusterChain> {
        let mut available = self.available.load(Ordering::Relaxed);
        while let Err(x) = self.available.compare_exchange_weak(
            available,
            available.checked_sub(amount).ok_or(Errno::ENOSPC)?,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            available = x;
        }

        let mut chain = ClusterChain {
            data: Vec::new(),
            len: 0,
        };
        let mut i = 0;
        loop {
            'l0: loop {
                if chain.len >= amount {
                    return Ok(chain);
                }

                // Pop a single cluster.
                let mut cur = self.bitmap[i].load(Ordering::Relaxed);
                let mut mask = cur ^ (cur & cur.wrapping_sub(1));
                if mask == 0 {
                    break 'l0;
                }
                while let Err(x) = self.bitmap[i].compare_exchange(
                    cur,
                    !mask,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    cur = x;
                    mask = cur ^ (cur & cur.wrapping_sub(1));
                    if mask == 0 {
                        break 'l0;
                    }
                }

                // Add the cluster to the chain.
                if let Err(_) = chain.try_reserve(1) {
                    self.free_chain(&chain);
                    return Err(Errno::ENOMEM);
                }
                chain.push(mask.trailing_zeros() + i as u32 * usize::BITS);
            }
            i = (i + 1) % self.bitmap.len();
        }
    }
}

#[derive(Clone, Copy)]
/// An iterable range in a [`ClusterChain`].
pub(super) struct ClusterRange<'a> {
    chain: &'a ClusterChain,
    data_start: usize,
    range_start: u32,
    length: u32,
}

impl<'a> IntoIterator for ClusterRange<'a> {
    type Item = u32;
    type IntoIter = ClusterIter<'a>;

    fn into_iter(self) -> ClusterIter<'a> {
        ClusterIter {
            chain: self.chain,
            range: self.chain.data[self.data_start].range.clone(),
            data_index: self.data_start,
            length: self.length,
        }
    }
}

#[derive(Clone)]
/// An iterator over a [`ClusterRange`].
pub(super) struct ClusterIter<'a> {
    chain: &'a ClusterChain,
    range: Range<u32>,
    data_index: usize,
    length: u32,
}

impl Iterator for ClusterIter<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.length == 0 {
            return None;
        }
        self.length -= 1;

        loop {
            if let Some(x) = self.range.next() {
                return Some(x);
            }
            self.data_index += 1;
            self.range = self.chain.data[self.data_index].range.clone();
        }
    }
}

#[derive(Clone)]
/// A range in a [`ClusterChain`].
struct ClusterLink {
    /// Offset in the file.
    offset: u32,
    /// Range of clusters on the media.
    range: Range<u32>,
}

/// Represents a chain of allocated clusters for a FAT file.
pub(super) struct ClusterChain {
    data: Vec<ClusterLink>,
    len: u32,
}

impl Debug for ClusterChain {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("ClusterChain {{ len: {}, data: [", self.len))?;
        for (i, link) in self.data.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            f.write_fmt(format_args!(
                "0x{:x}-0x{:x} @ {}",
                link.range.start,
                link.range.end - 1,
                link.offset,
            ))?;
        }
        f.write_str("] }")?;
        Ok(())
    }
}

impl ClusterChain {
    /// Make an empty cluster chain.
    pub(super) fn new() -> Self {
        Self {
            data: Vec::new(),
            len: 0,
        }
    }

    /// Get number of the cluster that is `offset` clusters into the file.
    pub(super) fn get(&self, offset: u32) -> Option<u32> {
        if self.len == 0 {
            return None;
        }
        self.range(offset, 1).into_iter().next()
    }

    /// Get an iterator for the range of clusters starting at `offset` with length `length`.
    pub(super) fn range<'a>(&'a self, offset: u32, length: u32) -> ClusterRange<'a> {
        debug_assert!(offset + length <= self.len);
        let index = match self
            .data
            .binary_search_by(|range| range.offset.cmp(&offset))
        {
            Ok(x) => x,
            Err(x) => x,
        };
        ClusterRange {
            chain: self,
            data_start: index,
            range_start: offset - self.data[index].offset,
            length,
        }
    }

    /// Get the number of entries used to represent this chain.
    pub(super) fn entries_len(&self) -> usize {
        self.data.len()
    }

    /// Get the length of this chain.
    pub(super) fn len(&self) -> u32 {
        self.len
    }

    /// Try to reserve memory for extending this chain.
    pub(super) fn try_reserve(&mut self, additional_ents: usize) -> Result<(), TryReserveError> {
        self.data.try_reserve(additional_ents)
    }

    /// Add a single cluster to this chain.
    pub(super) fn push(&mut self, cluster: u32) {
        if self
            .data
            .last()
            .map(|x| x.range.end == cluster)
            .unwrap_or(false)
        {
            self.data.last_mut().unwrap().range.end += 1;
        } else {
            self.data.push(ClusterLink {
                offset: self
                    .data
                    .last()
                    .map(|x| x.offset + x.range.len() as u32)
                    .unwrap_or(0),
                range: cluster..cluster + 1,
            });
        }
        self.len += 1;
    }

    /// Get the last cluster in this chain.
    pub(super) fn last(&self) -> Option<u32> {
        if self.len == 0 {
            return None;
        }
        Some(self.data.last().unwrap().range.end - 1)
    }

    /// Extend this chain by consuming another.
    pub(super) fn extend(&mut self, mut other: Self) {
        if self.data.len() == 0 {
            *self = other;
            return;
        }
        if self.data.last().unwrap().range.end == other.data[0].range.start {
            self.data.last_mut().unwrap().range.end = other.data.remove(0).range.end;
        }
        let add_offset = self
            .data
            .last()
            .map(|x| x.offset + x.range.len() as u32)
            .unwrap_or(0);
        let start = self.data.len();
        self.data.extend_from_slice(&other.data);
        for i in start..self.data.len() {
            self.data[i].offset += add_offset;
        }
        self.len += other.len;
    }

    /// Truncate the last number of clusters from this chain.
    pub(super) fn shorten(&mut self, amount: u32) {
        debug_assert!(amount <= self.len);
        self.len -= amount;

        // Remove the last clusters from the chain.
        let mut remaining = amount;
        while remaining > 0 && !self.data.is_empty() {
            let last = self.data.last_mut().unwrap();
            if last.range.len() as u32 <= remaining {
                remaining -= last.range.len() as u32;
                self.data.pop();
            } else {
                last.range.end -= remaining;
                remaining = 0;
            }
        }
    }

    /// Helper function to write to media using this cluster chain.
    pub(super) fn write(
        &self,
        fatfs: &FatFs,
        offset: u64,
        wdata: UserSlice<'_, u8>,
    ) -> EResult<()> {
        // Calculate the range of clusters that need to be accessed.
        let first_cluster = (offset >> fatfs.cluster_size_exp) as u32;
        let index = match self
            .data
            .binary_search_by(|range| range.offset.cmp(&first_cluster))
        {
            Ok(x) => x,
            Err(x) => x - 1,
        };

        // Iterate the ranges of clusters, accessing in as large chunks as possible.
        for range in &self.data[index..] {
            // The file offset of this range of clusters.
            let fileoff_start = (range.offset as u64) << fatfs.cluster_size_exp;
            let fileoff_end = fileoff_start
                + (((range.range.end - range.range.start) as u64) << fatfs.cluster_size_exp);

            // The range within that will be accessed.
            let access_start = fileoff_start.max(offset);
            let access_end = fileoff_end.min(offset + wdata.len() as u64);
            if access_start >= access_end {
                break;
            }

            // The offset within the data cluster area.
            let data_start = access_start - fileoff_start
                + ((range.range.start as u64) << fatfs.cluster_size_exp);

            // Actually access the media.
            fatfs.media.write(
                data_start + fatfs.data_offset,
                wdata.subslice((access_start - offset) as usize..(access_end - offset) as usize),
            )?;
        }

        Ok(())
    }

    /// Helper function to read from media using this cluster chain.
    pub(super) fn read(
        &self,
        fatfs: &FatFs,
        offset: u64,
        mut rdata: UserSliceMut<'_, u8>,
    ) -> EResult<()> {
        // Calculate the range of clusters that need to be accessed.
        let first_cluster = (offset >> fatfs.cluster_size_exp) as u32;
        let index = match self
            .data
            .binary_search_by(|range| range.offset.cmp(&first_cluster))
        {
            Ok(x) => x,
            Err(x) => x - 1,
        };

        // Iterate the ranges of clusters, accessing in as large chunks as possible.
        for range in &self.data[index..] {
            // The file offset of this range of clusters.
            let fileoff_start = (range.offset as u64) << fatfs.cluster_size_exp;
            let fileoff_end = fileoff_start
                + (((range.range.end - range.range.start) as u64) << fatfs.cluster_size_exp);

            // The range within that will be accessed.
            let access_start = fileoff_start.max(offset);
            let access_end = fileoff_end.min(offset + rdata.len() as u64);
            if access_start >= access_end {
                break;
            }

            // The offset within the data cluster area.
            let data_start = access_start - fileoff_start
                + ((range.range.start as u64) << fatfs.cluster_size_exp);

            // Actually access the media.
            fatfs.media.read(
                data_start + fatfs.data_offset,
                rdata
                    .subslice_mut((access_start - offset) as usize..(access_end - offset) as usize),
            )?;
        }

        Ok(())
    }

    /// Helper function to sync the cluster chain to the media.
    pub(super) fn sync(&self, fatfs: &FatFs) -> EResult<()> {
        for range in &self.data {
            fatfs.media.sync(
                (range.range.start as u64 + fatfs.data_offset) << fatfs.cluster_size_exp,
                ((range.range.end - range.range.start) as u64) << fatfs.cluster_size_exp,
            )?;
        }

        Ok(())
    }
}

impl<'a> IntoIterator for &'a ClusterChain {
    type Item = u32;
    type IntoIter = ClusterIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ClusterIter {
            chain: self,
            range: self.data.first().map(|x| x.range.clone()).unwrap_or(0..0),
            data_index: 0,
            length: self.len,
        }
    }
}
