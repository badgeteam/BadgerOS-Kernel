// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use chrono::{Datelike, TimeZone, Timelike, Utc};
use cluster::{ClusterAlloc, ClusterChain};
use spec::{Bpb, Dirent, Header16, Header32, LfnEnt, attr};

use crate::{
    badgelib::{
        time::Timespec,
        utf8::{StaticString, StringLike},
    },
    bindings::{
        device::HasBaseDevice,
        error::{EResult, Errno},
    },
    filesystem::{VNodeMtxInner, fatfs::spec::attr2, vfs::vnflags},
    kernel::sync::mutex::Mutex,
    mem::vmm::zeroes,
    process::usercopy::{UserSlice, UserSliceMut},
};

use super::{
    FSDRIVERS, MakeFileSpec, NodeType, Stat,
    media::Media,
    vfs::{VNode, VNodeOps, Vfs, VfsDriver, VfsOps, mflags::MFlags},
};
use super::{NAME_MAX, sysimpl::DentBuffer};
use core::{fmt::Debug, num::NonZeroU8, sync::atomic::Ordering};

mod cluster;
mod spec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Types of FAT filesystem.
enum FatType {
    Fat12,
    Fat16,
    Fat32,
}

/// Either a cluster chain or the region where the FAT12/FAT16 root directory is.
#[derive(Debug)]
enum FatFileStorage {
    /// The root directory for FAT12/FAT16, which is stored in a fixed location.
    Root16(u64),
    /// A chain of clusters; also used for FAT32 root directories.
    Clusters(ClusterChain),
}

/// A FAT file node.
/// Despite FAT not being designed for it, unlinked files can still be accessed by their VNode.
/// Note: Directories allow writing, which is used internally, and the outer VFS prevents the user from writing to directories.
#[derive(Debug)]
struct FatVNode {
    /// Where this file is stored in the media.
    storage: FatFileStorage,
    /// The size of the file.
    len: u32,
    /// Offset of the parent dirent, if any.
    dirent_disk_off: Mutex<Option<u64>>,
    /// Is a directory?
    is_dir: bool,
}

impl FatVNode {
    /// Write data to the file.
    pub fn writek(&self, arc_self: &Arc<VNode>, offset: u64, wdata: &[u8]) -> EResult<()> {
        self.write(arc_self, offset, UserSlice::new_kernel(wdata))
    }

    /// Read data from the file.
    pub fn readk(&self, arc_self: &Arc<VNode>, offset: u64, rdata: &mut [u8]) -> EResult<()> {
        self.read(arc_self, offset, UserSliceMut::new_kernel_mut(rdata))
    }

    /// Get the on-disk offset of some byte of the file.
    /// Primarily used for dirents.
    fn disk_offset_of(&self, arc_self: &Arc<VNode>, offset: u32) -> u64 {
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();
        match &self.storage {
            FatFileStorage::Root16(x) => x + offset as u64,
            FatFileStorage::Clusters(chain) => {
                let cluster = chain.get(offset >> fatfs.cluster_size_exp).unwrap();
                ((cluster as u64) << fatfs.cluster_size_exp)
                    + (offset & !((1u32 << fatfs.cluster_size_exp) - 1)) as u64
            }
        }
    }

    /// Helper to read LFN entries for some dirent.
    #[inline(always)]
    fn read_lfn(
        &self,
        arc_self: &Arc<VNode>,
        mut offset: u32,
        lfn_out: &mut impl StringLike,
    ) -> EResult<bool> {
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();
        if !fatfs.allow_lfn {
            return Ok(false);
        }

        let mut order = 1u8;
        while offset > 0 {
            offset -= 32;
            let mut dirent = [0u8; 32];
            self.readk(arc_self, offset as u64, &mut dirent)?;
            let mut dirent = LfnEnt::from(dirent);
            if dirent.attr != attr::LONG_NAME {
                break;
            }
            dirent.from_le();

            for &char in dirent.get_name().iter() {
                if dirent.order & 0x3f != order {
                    arc_self.vfs.check_eio_failed();
                    // Invalid LFN; ignore it.
                    return Ok(false);
                }
                if char == 0 {
                    if dirent.order & 0x40 != 0x40 {
                        // Invalid LFN; ignore it.
                        arc_self.vfs.check_eio_failed();
                        return Ok(false);
                    }
                    break;
                }
                if !lfn_out.push(unsafe { char::from_u32_unchecked(char as u32) }) {
                    // Out of memory; only short name can be used.
                    return Ok(false);
                }
            }
            if dirent.order & 0x40 == 0x40 {
                return Ok(true);
            }
            order += 1;
        }

        if lfn_out.len() != 0 {
            Err(Errno::EIO)
        } else {
            Ok(false)
        }
    }

    /// Helper function to iterate dirents.
    /// Stops iteration if `dirent_func` return `Err(_)` or `Ok(false)`.
    fn iter_dirents(
        &self,
        arc_self: &Arc<VNode>,
        read_lfn: bool,
        mut offset: u32,
        dirent_func: &mut dyn FnMut(u32, &Dirent, &str, Option<&str>) -> EResult<bool>,
    ) -> EResult<()> {
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();
        let mut lfn_buf = StaticString::<NAME_MAX>::new();

        // Iterate over raw directory entries, only calling `dirent_func` once for each valid dirent.
        // If the LFN name ends up being too long, then the 8.3 format name is used instead.
        while offset < self.len {
            let mut raw_dirent = [0u8; 32];
            self.readk(arc_self, offset as u64, &mut raw_dirent)?;
            let raw_dirent = Dirent::from(raw_dirent);
            if raw_dirent.name[0] == 0 {
                // No more allocated dirents.
                break;
            } else if raw_dirent.name[0] == 0xe5 {
                // Free dirent.
            } else if raw_dirent.attr & attr::VOLUME_ID == 0 {
                let mut dirent = raw_dirent;
                dirent.from_le();

                // Try to read preceding LFN dirents.
                let use_lfn = if read_lfn {
                    lfn_buf.clear();
                    self.read_lfn(arc_self, offset, &mut lfn_buf)?
                } else {
                    false
                };

                // Convert short filename.
                let mut sfn = StaticString::<12>::new();
                fatfs.short_name_to_str(&dirent.name, dirent.attr2, &mut sfn);

                // Check validity of dirent.
                if dirent.name != *b".          "
                    && dirent.name != *b"..         "
                    && !dirent.name.iter().all(|&x| {
                        fatfs.is_valid_short_char(unsafe { char::from_u32_unchecked(x as u32) })
                    })
                {
                    arc_self.vfs.check_eio_failed();
                } else if !(dirent_func(
                    offset,
                    &dirent,
                    sfn.as_ref(),
                    use_lfn.then_some(lfn_buf.as_ref()),
                )?) {
                    // Callback said to stop iterating.
                    break;
                }
            }

            offset += 32;
        }

        Ok(())
    }

    /// Converts a [`Dirent`] and a name to a [`super::Dirent`].
    fn convert_dirent(
        dirent_disk_off: u64,
        dirent_off: u32,
        dirent: &Dirent,
        name: &[u8],
    ) -> EResult<super::Dirent> {
        let mut name_copy = Vec::try_with_capacity(name.len())?;
        name_copy.resize(name.len(), 0);
        name_copy.copy_from_slice(name);

        let type_ = if dirent.attr & attr::DIRECTORY != 0 {
            NodeType::Directory
        } else {
            NodeType::Regular
        };

        Ok(super::Dirent {
            ino: 0,
            type_,
            name: name_copy.into(),
            dirent_disk_off,
            dirent_off: dirent_off as u64,
        })
    }

    /// Delete a dirent (doesn't mark clusters as free).
    fn delete_dirent(&mut self, arc_self: &Arc<VNode>, dirent_off: u32) -> EResult<()> {
        debug_assert!(dirent_off % 32 == 0);
        debug_assert!(dirent_off < self.len);

        // Check whether this is the last dirent.
        let is_last = if dirent_off + 32 < self.len {
            let mut tmp = [0u8];
            self.readk(arc_self, dirent_off as u64 + 32, &mut tmp)?;
            tmp[0] == 0
        } else {
            false
        };

        // Mark this dirent as free.
        let erase_val = [if is_last { 0u8 } else { 0xe5u8 }];
        self.writek(arc_self, dirent_off as u64, &erase_val)?;

        // Mark preceding LFN entries as free.
        let mut lfn_off = dirent_off;
        while lfn_off > 0 {
            lfn_off -= 32;

            // Read the previous dirent.
            let mut raw_lfn = [0u8; 32];
            self.readk(arc_self, lfn_off as u64, &mut raw_lfn)?;
            let mut lfn = LfnEnt::from(raw_lfn);
            lfn.from_le();

            // Stop if it is not an LFN entry.
            if raw_lfn[0] == 0xe5 || raw_lfn[0] == 0x00 || lfn.attr != attr::LONG_NAME {
                break;
            }

            // If it is an LFN entry, erase it.
            self.writek(arc_self, lfn_off as u64, &erase_val)?;
        }

        Ok(())
    }

    /// Find a range of free dirents or try to grow to fit.
    /// Returns the offset of the first dirent in the range.
    fn alloc_dirents(&mut self, arc_self: &Arc<VNode>, count: u32) -> EResult<u32> {
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();

        // Look for an existing gap large enough.
        let cur_cap = self.len / 32;
        let mut last_free = cur_cap;
        let mut found_count = 0u32;
        for i in 0..cur_cap {
            let mut tmp = [0xffu8];
            self.readk(arc_self, i as u64 * 32, &mut tmp)?;
            if tmp[0] == 0xe5 {
                found_count += 1;
                last_free = i;
            } else if tmp[0] == 0x00 {
                found_count += cur_cap - i;
                last_free = i;
            } else {
                found_count = 0;
                last_free = cur_cap;
            }
            if found_count >= count {
                return Ok((i + 1 - count) * 32);
            }
            if tmp[0] == 0 {
                break;
            }
        }

        // Try to resize the directory to fit.
        match &self.storage {
            FatFileStorage::Root16(_) => return Err(Errno::ENOSPC),
            FatFileStorage::Clusters(_) => (),
        };

        // Allocate enough clusters to this directory to make it fit.
        let cluster_size = 1u32 << fatfs.cluster_size_exp;
        let clusters_needed = cluster_size.div_ceil(count - (cur_cap - last_free));
        self.resize(
            arc_self,
            self.len
                .checked_add(clusters_needed.div_ceil(cluster_size))
                .ok_or(Errno::ENOSPC)? as u64,
        )?;

        Ok(last_free)
    }

    /// Determine whether a SFN already exists in this dir.
    fn sfn_is_duplicate(&self, arc_self: &Arc<VNode>, name: &[u8; 11]) -> EResult<bool> {
        let mut dup = false;
        self.iter_dirents(arc_self, false, 0, &mut |_off, ent, _sfn, _lfn| {
            if ent.name == *name {
                dup = true;
            }
            Ok(!dup)
        })?;
        Ok(dup)
    }

    /// Create a dirent.
    /// Returns the on-disk offset of the dirent.
    fn create_dirent(
        &mut self,
        arc_self: &Arc<VNode>,
        name: &str,
        is_dir: bool,
        is_rdonly: bool,
        first_cluster: Option<u32>,
        size: u32,
    ) -> EResult<u32> {
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();
        debug_assert!(name == FatFs::trim_name(name).unwrap());

        // Convert UTF-8 to 16-bit unicode string.
        let mut lfn = [0u16; 255];
        let lfn = {
            let mut lfn_len = 0usize;
            for char in name.chars() {
                if !FatFs::is_valid_long_char(char) {
                    return Err(Errno::EINVAL);
                } else if lfn_len >= lfn.len() {
                    return Err(Errno::ENAMETOOLONG);
                }
                lfn[lfn_len] = char as u16;
                lfn_len += 1;
            }
            &lfn[..lfn_len]
        };
        // Create matching short file name.
        let mut sfn = [0u8; 11];
        let mut attr2 = 0u8;
        let mut use_lfn = fatfs.long_to_short_name(lfn, &mut sfn, &mut attr2);

        // Increment the number suffix on the short name while it already exists.
        while self.sfn_is_duplicate(arc_self, &sfn)? {
            use_lfn = true;
            FatFs::increment_sfn_number(&mut sfn)?;
        }

        // Try to allocate dirents to store these.
        let lfn_ent_count = (lfn.len() as u32 + 12) / 13;
        let ent_count = 1 + use_lfn as u32 * lfn_ent_count;
        let free_range = self.alloc_dirents(arc_self, ent_count as u32)?;
        let dirent_off = free_range + 32 * (ent_count - 1);

        // Format the current date.
        let now = Timespec::now();
        let now = Utc.timestamp_nanos(now.sec as i64 * 1000000000 + now.nsec as i64);
        let ctime = spec::pack_date(
            now.year_ce().1.wrapping_sub(1980) as u8,
            now.month() as u8,
            now.day() as u8,
        );
        let ctime_2s = (now.second() / 2) as u16;
        let ctime_tenth = ((now.second() % 2 * 10) + now.nanosecond() / 100_000_000) as u8;

        // Format the new dirent.
        let first_cluster = first_cluster.map(|x| x + 2).unwrap_or(0);
        let mut dirent = Dirent {
            name: sfn,
            attr: is_dir as u8 * attr::DIRECTORY + is_rdonly as u8 * attr::READ_ONLY,
            attr2,
            ctime_tenth,
            ctime_2s,
            ctime,
            atime: ctime,
            first_cluster_hi: (first_cluster >> 16) as u16,
            mtime_2s: ctime_2s,
            mtime: ctime,
            first_cluster_lo: first_cluster as u16,
            size,
        };

        // Write the dirent.
        dirent.to_le();
        let dirent_bytes: [u8; 32] = dirent.into();
        let dirent_checksum = dirent
            .name
            .iter()
            .fold(0u8, |sum, &byte| sum.rotate_right(1).wrapping_add(byte));
        self.writek(arc_self, dirent_off as u64, &dirent_bytes)?;

        if use_lfn {
            // Write the preceding LFN entries.
            for i in 0..lfn_ent_count {
                // Get the part of the name that will be in this LFN entry.
                let mut name_part = [0xffffu16; 13];
                for x in 0..13 {
                    let lfn_idx = x + 13 * i as usize;
                    if lfn_idx >= lfn.len() {
                        name_part[x] = 0;
                        break;
                    }
                    name_part[x] = lfn[lfn_idx];
                }

                // Format the new LFN entry.
                let mut lfn_ent = LfnEnt::from([0u8; 32]);
                lfn_ent.attr = attr::LONG_NAME;
                lfn_ent.set_name(&name_part);
                lfn_ent.checksum = dirent_checksum;
                lfn_ent.order = i as u8 + 1;
                if i == lfn_ent_count - 1 {
                    lfn_ent.order |= 0x40;
                }

                // Write the new LFN entry.
                lfn_ent.to_le();
                let lfn_bytes: [u8; 32] = lfn_ent.into();
                self.writek(arc_self, (dirent_off - i * 32 - 32) as u64, &lfn_bytes)?;
            }
        }

        Ok(dirent_off)
    }

    /// Implementation of [`VNodeOps::make_file`].
    fn make_file_impl(
        &mut self,
        arc_self: &Arc<VNode>,
        name: &str,
        first_cluster: Option<u32>,
    ) -> EResult<(super::Dirent, Box<dyn VNodeOps>)> {
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();

        // Format the current date.
        let now = Timespec::now();
        let now = Utc.timestamp_nanos(now.sec as i64 * 1000000000 + now.nsec as i64);
        let ctime = spec::pack_date(
            now.year_ce().1.wrapping_sub(1980) as u8,
            now.month() as u8,
            now.day() as u8,
        );
        let ctime_2s = (now.second() / 2) as u16;
        let ctime_tenth = ((now.second() % 2 * 10) + now.nanosecond() / 100_000_000) as u8;

        if let Some(cluster) = first_cluster {
            let disk_off = fatfs.data_offset + ((cluster as u64) << fatfs.cluster_size_exp);

            // Make the `.` entry.
            let cluster = cluster + 2;
            let mut dirent = Dirent {
                name: *b".          ",
                attr: attr::DIRECTORY,
                attr2: 0,
                ctime_tenth,
                ctime_2s,
                ctime,
                atime: ctime,
                first_cluster_hi: (cluster >> 16) as u16,
                mtime_2s: ctime_2s,
                mtime: ctime,
                first_cluster_lo: cluster as u16,
                size: 0,
            };
            dirent.to_le();
            fatfs
                .media
                .writek(disk_off, &Into::<[u8; 32]>::into(dirent))?;

            // Make the `..` entry.
            if let Some(dirent_disk_off) = *self.dirent_disk_off.unintr_lock_shared() {
                // Copy that of this directory.
                let mut dirent = [0u8; 32];
                fatfs.media.readk(dirent_disk_off, &mut dirent)?;
                dirent[..11].copy_from_slice(b"..         ");
                fatfs.media.writek(disk_off + 32, &dirent)?;
            } else {
                // This is the root directory; use a hardcoded value.
                let mut dirent = Dirent {
                    name: *b"..         ",
                    attr: attr::DIRECTORY,
                    attr2: 0,
                    ctime_tenth: 0,
                    ctime_2s: 0,
                    ctime: 0,
                    atime: 0,
                    first_cluster_hi: 0,
                    mtime_2s: 0,
                    mtime: 0,
                    first_cluster_lo: 0,
                    size: 0,
                };
                dirent.to_le();
                let dirent = Into::<[u8; 32]>::into(dirent);
                fatfs.media.writek(disk_off + 32, &dirent)?;
            }
        }

        // Make the new dirent.
        let dirent_off = self.create_dirent(
            arc_self,
            name,
            first_cluster.is_some(),
            false,
            first_cluster,
            0,
        )?;

        let mut chain = ClusterChain::new();
        if let Some(cluster) = first_cluster {
            chain.push(cluster);
        }

        let ops = Box::try_new(FatVNode {
            storage: FatFileStorage::Clusters(chain),
            len: 0,
            dirent_disk_off: Mutex::new(Some(self.disk_offset_of(arc_self, dirent_off))),
            is_dir: first_cluster.is_some(),
        })?;
        let dirent = self.find_dirent(arc_self, name.as_bytes())?;
        Ok((dirent, Box::<dyn VNodeOps>::from(ops)))
    }
}

impl VNodeOps for FatVNode {
    fn write(&self, arc_self: &Arc<VNode>, offset: u64, wdata: UserSlice<'_, u8>) -> EResult<()> {
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();
        if offset.checked_add(wdata.len() as u64).ok_or(Errno::EIO)? > self.len as u64 {
            return Err(Errno::EIO);
        }
        match &self.storage {
            FatFileStorage::Root16(x) => fatfs.media.write(x + offset, wdata),
            FatFileStorage::Clusters(chain) => chain.write(&fatfs, offset, wdata),
        }
    }

    fn read(&self, arc_self: &Arc<VNode>, offset: u64, rdata: UserSliceMut<'_, u8>) -> EResult<()> {
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();
        if offset.checked_add(rdata.len() as u64).ok_or(Errno::EIO)? > self.len as u64 {
            return Err(Errno::EIO);
        }
        match &self.storage {
            FatFileStorage::Root16(x) => fatfs.media.read(x + offset, rdata),
            FatFileStorage::Clusters(chain) => chain.read(&fatfs, offset, rdata),
        }
    }

    fn resize(&mut self, arc_self: &Arc<VNode>, new_size: u64) -> EResult<()> {
        let old_size = self.len;
        let new_size = TryInto::<u32>::try_into(new_size).map_err(|_| Errno::ENOSPC)?;
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();
        let mut new_clusters =
            ((new_size + (1 << fatfs.cluster_size_exp) - 1) >> fatfs.cluster_size_exp) as u32;
        let dirent_disk_offset = self.dirent_disk_off.lock_shared()?;
        match &mut self.storage {
            FatFileStorage::Root16(_) => {
                // For FAT12/FAT16 root directory, resizing is not supported.
                return Err(Errno::ENOSPC);
            }
            FatFileStorage::Clusters(chain) => {
                if new_clusters > chain.len() {
                    // Allocate additional clusters.
                    let extra_chain = fatfs
                        .cluster_alloc
                        .alloc_chain(new_clusters - chain.len())?;
                    if let Err(x) = chain.try_reserve(extra_chain.entries_len()) {
                        fatfs.cluster_alloc.free_chain(&extra_chain);
                        return Err(x.into());
                    }

                    if let Some(dirent_disk_offset) = *dirent_disk_offset
                        && chain.len() == 0
                    {
                        // Modify first cluster offset.
                        let first_cluster = (extra_chain.get(0).unwrap() + 2).to_le_bytes();
                        fatfs
                            .media
                            .writek(dirent_disk_offset + 0x1a, &first_cluster[0..2])?;
                        fatfs
                            .media
                            .writek(dirent_disk_offset + 0x14, &first_cluster[2..4])?;
                    }

                    // Update the FAT entries.
                    // Note: If `FatFS::fat_set` fails, it will return EIO, causing the VFS to make this FS read-only.
                    let mut last_cluster = chain.last();
                    for cluster in &extra_chain {
                        if let Some(last_cluster) = last_cluster {
                            fatfs
                                .fat_set(last_cluster, FatValue::Next(cluster))
                                .map_err(|_| Errno::EIO)?;
                        }
                        last_cluster = Some(cluster);
                    }
                    fatfs
                        .fat_set(last_cluster.unwrap(), FatValue::Eoc)
                        .map_err(|_| Errno::EIO)?;

                    chain.extend(extra_chain);
                } else {
                    new_clusters = new_clusters.max(1);
                    if new_clusters < chain.len() {
                        // Free excess clusters.
                        // Note: If `FatFS::fat_set` fails, it will return EIO, causing the VFS to make this FS read-only.
                        for cluster in chain.range(new_clusters, chain.len() - new_clusters) {
                            fatfs.cluster_alloc.free(cluster);
                            fatfs
                                .fat_set(cluster, FatValue::Free)
                                .map_err(|_| Errno::EIO)?;
                        }
                        chain.shorten(chain.len() - new_clusters);
                    }
                }
            }
        }

        self.len = new_size;
        if let Some(dirent_disk_offset) = *dirent_disk_offset
            && arc_self.type_ == NodeType::Regular
        {
            // Update length, but only for regular files.
            let len = new_size.to_le_bytes();
            fatfs.media.writek(dirent_disk_offset + 0x1c, &len)?;
        }

        // Erase new bytes.
        if new_size > old_size {
            let zeroes = zeroes();
            let mut offset = old_size;
            while offset < new_size {
                let len = zeroes.len().min((new_size - offset) as usize);
                self.writek(arc_self, offset as u64, &zeroes[..len])?;
                offset += len as u32;
            }
        }

        Ok(())
    }

    fn find_dirent(&self, arc_self: &Arc<VNode>, name: &[u8]) -> EResult<super::Dirent> {
        let name = FatFs::trim_name_bytes(name);
        let mut res = Err(Errno::ENOENT);
        let res_ptr = &mut res;
        self.iter_dirents(arc_self, true, 0, &mut |off, dent, sfn, lfn| {
            let disk_off = self.disk_offset_of(arc_self, off);
            if let Some(lfn) = lfn
                && FatFs::name_equals(lfn.as_bytes(), name)
            {
                *res_ptr = Ok(Self::convert_dirent(disk_off, off, dent, lfn.as_bytes())?);
                Ok(false)
            } else if FatFs::name_equals(sfn.as_bytes(), name) {
                *res_ptr = Ok(Self::convert_dirent(disk_off, off, dent, sfn.as_bytes())?);
                Ok(false)
            } else {
                Ok(true)
            }
        })?;
        res
    }

    fn get_dirents(
        &self,
        arc_self: &Arc<VNode>,
        offset: u64,
        buffer: &mut DentBuffer<'_>,
    ) -> EResult<u64> {
        let mut offset = offset as u32;
        offset -= offset % 32;
        self.iter_dirents(arc_self, true, offset, &mut |off, dent, sfn, lfn| {
            if off < offset {
                Ok(true)
            } else {
                let disk_off = self.disk_offset_of(arc_self, off);
                let fits = Self::convert_dirent(disk_off, off, dent, lfn.unwrap_or(sfn).as_bytes())
                    .and_then(|x| buffer.push(x))?;
                if fits {
                    offset = off + 32;
                }
                Ok(fits)
            }
        })?;
        Ok(offset as u64)
    }

    fn unlink(
        &mut self,
        arc_self: &Arc<VNode>,
        name: &[u8],
        is_rmdir: bool,
        unlinked_vnode: Option<Arc<VNode>>,
    ) -> EResult<()> {
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();
        let ent = self.find_dirent(arc_self, name)?;

        // Get the FAT dirent.
        let mut fat_ent = [0u8; size_of::<Dirent>()];
        fatfs.media.readk(ent.dirent_disk_off, &mut fat_ent)?;
        let mut fat_ent = Dirent::from(fat_ent);
        fat_ent.from_le();
        let first_cluster =
            ((fat_ent.first_cluster_hi as u32) << 16) | (fat_ent.first_cluster_lo as u32);
        let mut chain = if let Some(first_cluster) = first_cluster.checked_sub(2) {
            Some(fatfs.read_chain(&arc_self.vfs, first_cluster)?)
        } else {
            None
        };

        // Determine whether removal is allowed.
        if is_rmdir {
            // Must be a directory.
            if fat_ent.attr & attr::DIRECTORY == 0 {
                return Err(Errno::ENOTDIR);
            }

            // The directory must be empty.
            let cluster_size = 1u32 << fatfs.cluster_size_exp;
            let chain = chain.as_mut().unwrap();
            for i in 0..chain.len() * cluster_size / 32 {
                let mut name = [0u8; 11];
                chain.read(
                    &fatfs,
                    i as u64 * 32,
                    UserSliceMut::new_kernel_mut(&mut name),
                )?;
                if name[0] == 0 {
                    break;
                } else if name[0] != 0xe5 && name != *b".          " && name != *b"..         " {
                    return Err(Errno::ENOTEMPTY);
                }
            }
        } else {
            // Must be a regular file.
            if fat_ent.attr & attr::DIRECTORY != 0 {
                return Err(Errno::EISDIR);
            }
        }

        if let Some(unlinked_vnode) = &unlinked_vnode {
            // Mark it as not having a dirent on the VNode, if it is currently open.
            let fat_vnode = unsafe {
                &*(unlinked_vnode.mtx.data().ops.as_ref() as *const dyn VNodeOps as *const FatVNode)
            };
            *fat_vnode.dirent_disk_off.unintr_lock() = None;
            unlinked_vnode
                .flags
                .fetch_or(vnflags::REMOVED, Ordering::Relaxed);
        }

        // Either way, mark the clusters as free in the FAT.
        // If the VNode was still open, it'll only have them reserved in memory.
        if let Some(chain) = &chain {
            for cluster in chain {
                fatfs.fat_set(cluster, FatValue::Free)?;
            }
        }

        if unlinked_vnode.is_none() || is_rmdir {
            // If not open, mark the chain as free.
            if let Some(chain) = &chain {
                fatfs.cluster_alloc.free_chain(chain);
            }
        }

        self.delete_dirent(arc_self, ent.dirent_off as u32)
    }

    fn link(&mut self, _arc_self: &Arc<VNode>, _name: &[u8], _inode: &VNode) -> EResult<()> {
        Err(Errno::EPERM)
    }

    fn make_file(
        &mut self,
        arc_self: &Arc<VNode>,
        name: &[u8],
        spec: MakeFileSpec,
    ) -> EResult<(super::Dirent, Box<dyn VNodeOps>)> {
        let name = FatFs::trim_name(str::from_utf8(name).map_err(|_| Errno::EINVAL)?)
            .ok_or(Errno::ENOENT)?;
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();

        let first_cluster = match spec {
            MakeFileSpec::Directory => Some({
                let cluster = fatfs.cluster_alloc.alloc()?;
                if let Err(x) = fatfs.fat_set(cluster, FatValue::Eoc) {
                    fatfs.cluster_alloc.free(cluster);
                    return Err(x);
                }
                cluster
            }),
            MakeFileSpec::Regular => None,
            _ => return Err(Errno::EPERM),
        };

        let mut res = self.make_file_impl(arc_self, name, first_cluster);
        if let Some(cluster) = first_cluster
            && res.is_err()
        {
            if let Err(x) = fatfs.fat_set(cluster, FatValue::Free) {
                res = Err(x);
            }
            fatfs.cluster_alloc.free(cluster);
        }

        res
    }

    fn rename(
        &mut self,
        arc_self: &Arc<VNode>,
        old_name: &[u8],
        new_name: &[u8],
    ) -> EResult<super::Dirent> {
        let new_name = FatFs::trim_name(str::from_utf8(new_name).map_err(|_| Errno::EINVAL)?)
            .ok_or(Errno::ENOENT)?;

        // Find the old dirent.
        let mut old_dent = None;
        self.iter_dirents(arc_self, true, 0, &mut |off, dent, sfn, lfn| {
            if FatFs::name_equals(old_name, lfn.unwrap_or(sfn).as_bytes()) {
                old_dent = Some((off, *dent));
                Ok(false)
            } else {
                Ok(true)
            }
        })?;
        let (old_dent_off, old_dent) = old_dent.ok_or(Errno::ENOENT)?;

        // Replace the dirent.
        self.delete_dirent(arc_self, old_dent_off)?;
        let first_cluster =
            ((old_dent.first_cluster_hi as u32) << 16) + old_dent.first_cluster_lo as u32;
        let new_dent_off = self.create_dirent(
            arc_self,
            new_name,
            old_dent.attr & attr::DIRECTORY != 0,
            old_dent.attr & attr::READ_ONLY != 0,
            first_cluster.checked_sub(2),
            old_dent.size,
        )?;

        // New dirent conv.
        let mut new_dent = [0u8; 32];
        self.readk(arc_self, new_dent_off as u64, &mut new_dent)?;
        let mut new_dent = Dirent::from(new_dent);
        new_dent.from_le();
        let dirent = Self::convert_dirent(
            self.disk_offset_of(arc_self, new_dent_off),
            new_dent_off,
            &new_dent,
            new_name.as_bytes(),
        )?;

        Ok(dirent)
    }

    fn readlink(&self, _arc_self: &Arc<VNode>) -> EResult<Box<[u8]>> {
        Err(Errno::EINVAL)
    }

    fn stat(&self, arc_self: &Arc<VNode>) -> EResult<Stat> {
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();
        let guard = self.dirent_disk_off.lock_shared()?;
        let epoch = Utc.timestamp_nanos(0);

        // Read the dirent, if present.
        let mut dirent = [0u8; 32];
        if let Some(dirent_disk_off) = *guard {
            fatfs.media.readk(dirent_disk_off, &mut dirent)?;
        }
        let mut dirent = Dirent::from(dirent);
        dirent.from_le();

        // Convert creation time.
        let sec = dirent.ctime_2s as u32 * 2 + dirent.ctime_tenth as u32 / 10;
        let (year, month, day) = spec::unpack_date(dirent.ctime);
        let ctime = Utc
            .with_ymd_and_hms(
                year as i32 + 1980,
                month as u32,
                day as u32,
                sec / 3600,
                sec / 60 % 60,
                sec % 60,
            )
            .single()
            .unwrap_or(epoch);

        // Convert access time.
        let (year, month, day) = spec::unpack_date(dirent.atime);
        let atime = Utc
            .with_ymd_and_hms(year as i32 + 1980, month as u32, day as u32, 0, 0, 0)
            .single()
            .unwrap_or(epoch);

        // Convert modification time.
        let sec = dirent.mtime_2s as u32 * 2;
        let (year, month, day) = spec::unpack_date(dirent.mtime);
        let mtime = Utc
            .with_ymd_and_hms(
                year as i32 + 1980,
                month as u32,
                day as u32,
                sec / 3600,
                sec / 60 % 60,
                sec % 60,
            )
            .single()
            .unwrap_or(epoch);

        // Determine how long the cluster chain actually is.
        let blocks = match &self.storage {
            FatFileStorage::Root16(_) => self.len.div_ceil(512),
            FatFileStorage::Clusters(cluster_chain) => {
                cluster_chain.len() << (fatfs.cluster_size_exp - 9)
            }
        } as u64;

        Ok(Stat {
            dev: fatfs
                .media
                .device()
                .map(|dev| ((Into::<u32>::into(dev.id()) as u64) << 32) | dev.class() as u64)
                .unwrap_or(0),
            ino: 0,
            mode: 0777,
            nlink: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
            size: self.len as u64,
            blksize: 1u64 << fatfs.cluster_size_exp,
            blocks,
            atim: Timespec {
                sec: atime.timestamp() as u64,
                nsec: atime.nanosecond() as u32,
            },
            mtim: Timespec {
                sec: mtime.timestamp() as u64,
                nsec: mtime.nanosecond() as u32,
            },
            ctim: Timespec {
                sec: ctime.timestamp() as u64,
                nsec: ctime.nanosecond() as u32,
            },
        })
    }

    fn get_inode(&self) -> u64 {
        unimplemented!()
    }

    fn get_size(&self, _arc_self: &Arc<VNode>) -> u64 {
        self.len as u64
    }

    fn get_type(&self, _arc_self: &Arc<VNode>) -> NodeType {
        if self.is_dir {
            NodeType::Directory
        } else {
            NodeType::Regular
        }
    }

    fn sync(&self, arc_self: &Arc<VNode>) -> EResult<()> {
        let fatfs = arc_self.vfs.get_ops_as::<FatFs>();
        match &self.storage {
            FatFileStorage::Root16(x) => fatfs.media.sync(*x, self.len as u64),
            FatFileStorage::Clusters(chain) => chain.sync(&fatfs),
        }
    }

    fn close(&mut self, vnode_self: &VNode) {
        if let FatFileStorage::Clusters(chain) = &self.storage
            && self.dirent_disk_off.unintr_lock_shared().is_none()
            && vnode_self.is_vfs_root().is_none()
        {
            // If this VNode was unlinked, mark the clusters as free now.
            let fatfs = vnode_self.vfs.get_ops_as::<FatFs>();
            fatfs.cluster_alloc.free_chain(chain);
        }
    }
}

/// FAT entry values.
#[derive(PartialEq, Eq, Clone, Copy)]
enum FatValue {
    /// The next cluster in the chain.
    Next(u32),
    /// The end of the chain.
    Eoc,
    /// Free cluster.
    Free,
    /// Bad cluster.
    Bad,
}

/// A mounted FAT filesystem.
struct FatFs {
    /// The media this filesystem is mounted on.
    media: Media,
    /// The type of FAT filesystem (FAT12, FAT16, or FAT32).
    fat_type: FatType,
    /// Whether the long filename extension is enabled.
    allow_lfn: bool,
    /// Log-base 2 of the cluster size in bytes.
    cluster_size_exp: u32,
    /// Log-base 2 of the sector size in bytes.
    sector_size_exp: u32,
    /// The number of sectors per FAT.
    sectors_per_fat: u32,
    /// The number of clusters in the filesystem.
    cluster_count: u32,
    /// The cluster allocator for this filesystem.
    cluster_alloc: ClusterAlloc,
    /// Byte offset of the first data cluster.
    data_offset: u64,
    /// Sector offset of the first FAT.
    fat_sector: u32,
    /// Number of FATs.
    fat_count: u8,
    /// Active FAT index.
    active_fat: u8,
    /// Whether to mirror the FATs.
    mirror_fats: bool,
    /// Start cluster of the root directory, if FAT32.
    root_dir_cluster: u32,
    /// Offset of the root directory, if FAT12/FAT16.
    legacy_root_sector: u32,
    /// Size of the root directory in entries, if FAT12/FAT16.
    legacy_root_size: u32,
    /// Mutex used to protect FAT12 read-modify-write.
    fat12_mutex: Mutex<()>,
}

impl Debug for FatFs {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FatFS")
            .field("media", &self.media)
            .field("fat_type", &self.fat_type)
            .field("allow_lfn", &self.allow_lfn)
            .field("cluster_size_exp", &self.cluster_size_exp)
            .field("sector_size_exp", &self.sector_size_exp)
            .field("sectors_per_fat", &self.sectors_per_fat)
            .field("cluster_count", &self.cluster_count)
            .field("cluster_alloc", &())
            .field("data_offset", &self.data_offset)
            .field("fat_sector", &self.fat_sector)
            .field("fat_count", &self.fat_count)
            .field("active_fat", &self.active_fat)
            .field("mirror_fats", &self.mirror_fats)
            .field("root_dir_cluster", &self.root_dir_cluster)
            .field("legacy_root_sector", &self.legacy_root_sector)
            .field("legacy_root_size", &self.legacy_root_size)
            .field("fat12_mutex", &())
            .finish()
    }
}

impl FatFs {
    /// Whether a character is valid in a short name entry.
    /// Rejects lower-case characters; they are stored upper-case on disk.
    /// TODO: Subject to get support for non-ASCII encodings, which would allow more values.
    pub fn is_valid_short_char(&self, value: char) -> bool {
        if value as u32 >= 0x7f {
            return false;
        }
        match value as u8 {
            b'"' => false,
            b'*' => false,
            b'+' => false,
            b',' => false,
            b'.' => false,
            b'/' => false,
            b':' => false,
            b';' => false,
            b'<' => false,
            b'=' => false,
            b'>' => false,
            b'?' => false,
            b'[' => false,
            b'\\' => false,
            b']' => false,
            b'|' => false,
            x => x >= 0x20,
        }
    }

    /// Whether a character is valid in a long name entry.
    pub fn is_valid_long_char(value: char) -> bool {
        if value as u32 > 0xff {
            return true;
        }
        match value as u8 {
            b'"' => false,
            b'*' => false,
            b',' => false,
            b'/' => false,
            b':' => false,
            b'<' => false,
            b'>' => false,
            b'?' => false,
            b'\\' => false,
            b'|' => false,
            0x7f => false,
            x => x >= 0x20,
        }
    }

    /// Convert long char to equivalent short char.
    /// Returns [`None`] if it should be implicitly removed.
    pub fn long_to_short_char(&self, long: u16) -> Option<NonZeroU8> {
        if long == ' ' as u16 {
            None
        } else if self.is_valid_short_char(unsafe { char::from_u32_unchecked(long as u32) }) {
            Some(unsafe { NonZeroU8::new_unchecked((long as u8).to_ascii_uppercase()) })
        } else {
            Some(unsafe { NonZeroU8::new_unchecked(b'_') })
        }
    }

    /// Determines whether a short name is suffixed by ~number.
    pub fn is_number_suffixed(short_name: &[u8; 11]) -> Option<u32> {
        // Ignore the extension because it's not where the ~number suffix goes.
        let short_name = &short_name[..8];
        // Remove trailing spaces.
        let short_name = &short_name[..short_name
            .iter()
            .rposition(|x| *x != b' ')
            .unwrap_or(short_name.len() - 1)
            + 1];

        // Find last tilde.
        let tilde = short_name.iter().rposition(|x| *x == b'~')?;
        let mut number = 0u32;

        // Parse the number after the tilde.
        for &x in &short_name[tilde + 1..] {
            if x < b'0' || x > b'9' {
                return None; // Invalid character after tilde.
            }
            number = number * 10 + (x - b'0') as u32;
        }

        Some(number)
    }

    /// Convert a long name into a short one.
    /// If any information is lost, the output name will be suffixed with `~1`.
    /// Returns whether an LFN is needed.
    pub fn long_to_short_name(
        &self,
        mut long_name: &[u16],
        short_out: &mut [u8; 11],
        attr2_out: &mut u8,
    ) -> bool {
        short_out.fill(b' ');
        let mut info_lost = false;

        // Remove leading dots.
        while long_name.first().cloned() == Some('.' as u16) {
            long_name = &long_name[1..];
        }

        // Set extension.
        let mut ext_lc = false;
        let mut ext_uc = false;
        let ext = long_name.iter().rposition(|x| *x == '.' as u16);
        if let Some(ext) = ext {
            let mut i = 0;
            for &char in &long_name[ext + 1..] {
                if char >= b'a' as u16 && char <= b'z' as u16 {
                    ext_lc = true;
                } else if char >= b'A' as u16 && char <= b'Z' as u16 {
                    ext_uc = true;
                }
                if let Some(char) = self.long_to_short_char(char) {
                    let char = char.into();
                    if i < 3 {
                        short_out[8 + i] = char;
                        i += 1;
                    } else {
                        info_lost = true;
                    }
                } else {
                    info_lost = true;
                }
            }
        }

        // Set name.
        let mut name_lc = false;
        let mut name_uc = false;
        let mut i = 0;
        for &char in &long_name[..ext.unwrap_or(long_name.len())] {
            if char >= b'a' as u16 && char <= b'z' as u16 {
                name_lc = true;
            } else if char >= b'A' as u16 && char <= b'Z' as u16 {
                name_uc = true;
            }
            if let Some(char) = self.long_to_short_char(char) {
                let char = char.into();
                if i < 8 {
                    short_out[i] = char;
                    i += 1;
                } else {
                    info_lost = true;
                }
            } else {
                info_lost = true;
            }
        }

        let needs_lfn = info_lost || (name_lc && name_uc) || (ext_lc && ext_uc);

        // Suffix the output name with ~1 if info is lost and it is not already suffixed with ~number.
        if info_lost && Self::is_number_suffixed(short_out).is_none() {
            short_out[6] = b'~';
            short_out[7] = b'1';
        }
        if needs_lfn {
            *attr2_out = 0;
        } else {
            *attr2_out = name_lc as u8 * attr2::LC_NAME + ext_lc as u8 * attr2::LC_EXT;
        }

        needs_lfn
    }

    /// Convert a short name into a string.
    /// Returns whether `name_out` had enough capacity to store it.
    pub fn short_name_to_str(
        &self,
        short: &[u8; 11],
        attr2: u8,
        name_out: &mut impl StringLike,
    ) -> bool {
        // Find portion of name that is not just spaces.
        let name_end = short[..8]
            .iter()
            .rposition(|&x| x != b' ')
            .map_or(0, |x| x + 1);
        let ext_end = short[8..11]
            .iter()
            .rposition(|&x| x != b' ')
            .map_or(0, |x| x + 1);

        for &x in &short[..name_end] {
            let mut char = unsafe { char::from_u32_unchecked(x as u32) };
            if attr2 & attr2::LC_NAME != 0 {
                char = char.to_ascii_lowercase();
            }
            if !name_out.push(char) {
                return false;
            }
        }

        if ext_end > 0 {
            if !name_out.push('.') {
                return false;
            }
            for &x in &short[8..8 + ext_end] {
                let mut char = unsafe { char::from_u32_unchecked(x as u32) };
                if attr2 & attr2::LC_EXT != 0 {
                    char = char.to_ascii_lowercase();
                }
                if !name_out.push(char) {
                    return false;
                }
            }
        }

        true
    }

    /// Trim beginning and end off of names according to FAT rules.
    /// Operates on a UTF-8 string slice.
    fn trim_name<'a>(name: &'a str) -> Option<&'a str> {
        let tmp = name
            .trim_ascii_start()
            .trim_end_matches(|x| x == ' ' || x == '.');
        if tmp.chars().into_iter().all(|x| x == '.') {
            return None;
        }
        Some(tmp)
    }

    /// Trim beginning and end off of names according to FAT rules.
    /// Operates on bytes, some of which may be ASCII, instead of UTF-8.
    fn trim_name_bytes<'a>(mut name: &'a [u8]) -> &'a [u8] {
        while let Some(x) = name.first()
            && *x == b' '
        {
            name = &name[1..];
        }
        while let Some(x) = name.last()
            && (*x == b' ' || *x == b'.')
        {
            name = &name[..name.len() - 1];
        }
        name
    }

    /// Compares two names for equality by FAT rules.
    /// Assumes that the names are already trimmed.
    fn name_equals(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        for i in 0..a.len() {
            if !a[i].eq_ignore_ascii_case(&b[i]) {
                return false;
            }
        }
        true
    }

    /// Write the FAT next pointer for a cluster.
    fn fat_set_impl(self: &FatFs, fat_offset: u64, cluster: u32, value: u32) -> EResult<()> {
        match self.fat_type {
            FatType::Fat12 => {
                let mut bytes = [0u8; 2];
                self.media
                    .readk(fat_offset + (cluster as u64 * 3 / 2), &mut bytes)?;

                if cluster & 1 == 0 {
                    bytes[0] = value as u8;
                    bytes[1] = (bytes[1] & 0xf0) | ((value >> 8) as u8 & 0x0f);
                } else {
                    bytes[0] = (bytes[0] & 0x0f) | (value << 4) as u8;
                    bytes[1] = (value >> 4) as u8;
                }

                self.media
                    .writek(fat_offset + (cluster as u64 * 3 / 2), &bytes)?;
            }
            FatType::Fat16 => {
                let bytes = (value as u16).to_le_bytes();
                self.media
                    .writek(fat_offset + (cluster as u64 * 2), &bytes)?;
            }
            FatType::Fat32 => {
                // FAT requires preserving the upper bits, but nothing actually uses them so IDC.
                let bytes = value.to_le_bytes();
                self.media
                    .writek(fat_offset + (cluster as u64 * 4), &bytes)?;
            }
        }
        Ok(())
    }

    /// Write the FAT next pointer for a cluster.
    fn fat_set(&self, cluster: u32, value: FatValue) -> EResult<()> {
        debug_assert!(cluster < self.cluster_count);
        let cluster = cluster + 2;
        let value = match value {
            FatValue::Next(x) => {
                if x >= self.cluster_count {
                    return Err(Errno::EIO);
                }
                x + 2
            }
            FatValue::Eoc => 0x0fff_ffff,
            FatValue::Free => 0x0000_0000,
            FatValue::Bad => 0x0fff_fff7,
        };
        if self.mirror_fats {
            for i in 0..self.fat_count {
                self.fat_set_impl(
                    (self.fat_sector as u64 + i as u64 * self.sectors_per_fat as u64)
                        << self.sector_size_exp,
                    cluster,
                    value,
                )?;
            }
            Ok(())
        } else {
            self.fat_set_impl(
                (self.fat_sector as u64) << self.sector_size_exp,
                cluster,
                value,
            )
        }
    }

    /// Read the FAT next pointer for a cluster.
    fn fat_get(&self, cluster: u32) -> EResult<FatValue> {
        debug_assert!(cluster < self.cluster_count);
        let cluster = cluster + 2;
        let fat_offset = (self.fat_sector as u64
            + self.active_fat as u64 * self.sectors_per_fat as u64)
            << self.sector_size_exp;
        let value = match self.fat_type {
            FatType::Fat12 => {
                // Read FAT12 entry.
                let _guard = self.fat12_mutex.lock_shared();
                let mut bytes = [0u8; 2];
                self.media
                    .readk(fat_offset + (cluster as u64 * 3 / 2), &mut bytes)?;
                let tmp = if cluster & 1 == 0 {
                    bytes[0] as u16 | ((bytes[1] as u16 & 0x0f) << 8)
                } else {
                    (bytes[0] >> 4) as u16 | ((bytes[1] as u16) << 4)
                };
                if tmp >= 0xff7 {
                    tmp as u32 + 0x0fff_f000
                } else {
                    tmp as u32
                }
            }
            FatType::Fat16 => {
                // Read FAT16 entry.
                let mut bytes = [0u8; 2];
                self.media
                    .readk(fat_offset + (cluster as u64 * 2), &mut bytes)?;
                let tmp = u16::from_le_bytes(bytes);
                if tmp >= 0xfff7 {
                    tmp as u32 + 0x0fff_0000
                } else {
                    tmp as u32
                }
            }
            FatType::Fat32 => {
                // Read FAT32 entry.
                let mut bytes = [0u8; 4];
                self.media
                    .readk(fat_offset + (cluster as u64 * 4), &mut bytes)?;
                u32::from_le_bytes(bytes) & 0x0fff_ffff
            }
        };
        match value {
            0 => Ok(FatValue::Free),
            0x0fff_fff7 => Ok(FatValue::Bad),
            0x0fff_ffff => Ok(FatValue::Eoc),
            x if x >= 2 && x < self.cluster_count + 2 => Ok(FatValue::Next(x - 2)),
            _ => Err(Errno::EIO),
        }
    }

    /// Try to read a cluster chain.
    fn read_chain(&self, self_arc: &Arc<Vfs>, start_cluster: u32) -> EResult<ClusterChain> {
        let mut chain = ClusterChain::new();

        let mut cluster = start_cluster;
        loop {
            chain.try_reserve(1)?;
            chain.push(cluster);
            match self.fat_get(cluster)? {
                FatValue::Next(x) => cluster = x,
                FatValue::Eoc => break,
                _ => {
                    self_arc.check_eio_failed();
                    break;
                }
            }
        }

        Ok(chain)
    }

    /// Increment the number suffix on a short filename.
    fn increment_sfn_number(sfn: &mut [u8; 11]) -> EResult<()> {
        let mut num = Self::is_number_suffixed(sfn).unwrap_or(0);

        // Increment the number, return EEXIST if it overflows (9999999 -> 0).
        if num >= 9999999 {
            return Err(Errno::EEXIST);
        }
        num += 1;

        // Stringify the new number in a stack-allocated buffer.
        let mut tmp = [0u8; 8];
        let mut tmp_len = 0usize;
        // Convert digits starting from least significant.
        while num > 0 {
            tmp[7 - tmp_len] = (num % 10) as u8 + b'0';
            tmp_len += 1;
            num /= 10;
        }
        let tmp = &tmp[8 - tmp_len..];

        // Copy the buffer back into the name.
        sfn[7 - tmp.len()] = b'~';
        sfn[7 - tmp.len()..8].copy_from_slice(tmp);

        Ok(())
    }
}

impl VfsOps for FatFs {
    fn media(&self) -> Option<&Media> {
        Some(&self.media)
    }

    fn uses_inodes(&self) -> bool {
        false
    }

    fn open_root(&self, self_arc: &Arc<Vfs>) -> EResult<Box<dyn VNodeOps>> {
        if self.fat_type == FatType::Fat32 {
            let chain = self.read_chain(self_arc, self.root_dir_cluster)?;
            let len = chain.len() << self.cluster_size_exp;
            Ok(Box::<dyn VNodeOps>::from(Box::try_new(FatVNode {
                storage: FatFileStorage::Clusters(chain),
                len,
                dirent_disk_off: Mutex::new(None),
                is_dir: true,
            })?))
        } else {
            Ok(Box::<dyn VNodeOps>::from(Box::try_new(FatVNode {
                storage: FatFileStorage::Root16(
                    (self.legacy_root_sector as u64) << self.sector_size_exp,
                ),
                len: self.legacy_root_size * 32,
                dirent_disk_off: Mutex::new(None),
                is_dir: true,
            })?))
        }
    }

    fn open(
        &self,
        self_arc: &Arc<Vfs>,
        cached_dirent: &super::Dirent,
    ) -> EResult<Box<dyn VNodeOps>> {
        // Read FAT dirent from disk.
        let mut dirent = [0u8; size_of::<Dirent>()];
        self.media
            .readk(cached_dirent.dirent_disk_off, &mut dirent)?;
        let mut dirent = Dirent::from(dirent);

        // Read the cluster chain.
        dirent.from_le();
        let start_cluster =
            ((dirent.first_cluster_hi as u32) << 16) | dirent.first_cluster_lo as u32;
        let chain = self.read_chain(self_arc, start_cluster.checked_sub(2).ok_or(Errno::EIO)?)?;

        let is_dir = (dirent.attr & spec::attr::DIRECTORY) != 0;
        let chain_len = chain.len();
        Ok(Box::<dyn VNodeOps>::from(Box::try_new(FatVNode {
            storage: FatFileStorage::Clusters(chain),
            len: if is_dir {
                chain_len << self.cluster_size_exp
            } else {
                dirent.size
            },
            dirent_disk_off: Mutex::new(Some(cached_dirent.dirent_disk_off)),
            is_dir,
        })?))
    }

    fn rename(
        &self,
        _self_arc: &Arc<Vfs>,
        old_dir: &Arc<VNode>,
        old_name: &[u8],
        old_mutexinner: &mut VNodeMtxInner,
        new_dir: &Arc<VNode>,
        new_name: &[u8],
        new_mutexinner: &mut VNodeMtxInner,
    ) -> EResult<super::Dirent> {
        let new_name = FatFs::trim_name(str::from_utf8(new_name).map_err(|_| Errno::EINVAL)?)
            .ok_or(Errno::ENOENT)?;

        let old_ops =
            unsafe { &mut *(old_mutexinner.ops.as_mut() as *mut dyn VNodeOps as *mut FatVNode) };
        let new_ops =
            unsafe { &mut *(new_mutexinner.ops.as_mut() as *mut dyn VNodeOps as *mut FatVNode) };

        // Find the old dirent.
        let mut old_dent = None;
        old_ops.iter_dirents(old_dir, true, 0, &mut |off, dent, sfn, lfn| {
            if FatFs::name_equals(old_name, lfn.unwrap_or(sfn).as_bytes()) {
                old_dent = Some((off, *dent));
                Ok(false)
            } else {
                Ok(true)
            }
        })?;
        let (old_dent_off, old_dent) = old_dent.ok_or(Errno::ENOENT)?;

        // Replace the dirent.
        old_ops.delete_dirent(old_dir, old_dent_off)?;
        let first_cluster =
            ((old_dent.first_cluster_hi as u32) << 16) + old_dent.first_cluster_lo as u32;
        let new_dent_off = new_ops.create_dirent(
            new_dir,
            new_name,
            old_dent.attr & attr::DIRECTORY != 0,
            old_dent.attr & attr::READ_ONLY != 0,
            first_cluster.checked_sub(2),
            old_dent.size,
        )?;

        // New dirent conv.
        let mut new_dent = [0u8; 32];
        new_ops.readk(new_dir, new_dent_off as u64, &mut new_dent)?;
        let mut new_dent = Dirent::from(new_dent);
        new_dent.from_le();
        let dirent = FatVNode::convert_dirent(
            new_ops.disk_offset_of(new_dir, new_dent_off),
            new_dent_off,
            &new_dent,
            new_name.as_bytes(),
        )?;

        Ok(dirent)
    }
}

/// The FAT filesystem driver.
struct FatFsDriver {
    /// Whether the long filename extension is enabled.
    allow_lfn: bool,
}

impl VfsDriver for FatFsDriver {
    fn detect(&self, media: &Media) -> EResult<bool> {
        // Read the BPB.
        let mut bpb = [0u8; size_of::<Bpb>()];
        media.readk(0, &mut bpb)?;
        let mut bpb = Bpb::from(bpb);
        bpb.from_le();

        // Checked signature #1: valid sector size.
        // Sector size is an integer power of 2 from 512 to 4096.
        if bpb.bytes_per_sector < 512
            || bpb.bytes_per_sector > 4096
            || bpb.bytes_per_sector.count_ones() != 1
        {
            return Ok(false);
        }

        // Checked signature #2: FAT BPB signature bytes.
        let mut tmp = [0u8; 2];
        media.readk(510, &mut tmp)?;
        if tmp != [0x55, 0xaa] {
            return Ok(false);
        }

        // Both minor signature checks passed, this filesystem is probably FAT.
        Ok(true)
    }

    fn mount(&self, media: Option<Media>, _mflags: MFlags) -> EResult<Box<dyn VfsOps>> {
        // Read the BPB.
        let media = media.ok_or(Errno::ENODEV)?;
        let mut bpb = [0u8; size_of::<Bpb>()];
        media.readk(0, &mut bpb)?;
        let mut bpb = Bpb::from(bpb);
        bpb.from_le();

        if bpb.bytes_per_sector < 512
            || bpb.bytes_per_sector > 4096
            || bpb.bytes_per_sector.count_ones() != 1
        {
            return Err(Errno::EIO);
        }
        if bpb.sectors_per_cluster == 0 {
            return Err(Errno::EIO);
        }

        let mut header16 = [0u8; size_of::<Header16>()];
        let mut header32 = [0u8; size_of::<Header32>()];

        // Read the header.
        let sector_count = if bpb.sector_count_32 != 0 {
            media.readk(size_of::<Bpb>() as u64, &mut header32)?;
            media.readk(
                size_of::<Bpb>() as u64 + size_of::<Header32>() as u64,
                &mut header32,
            )?;
            bpb.sector_count_32
        } else {
            media.readk(size_of::<Bpb>() as u64, &mut header16)?;
            bpb.sector_count_16 as u32
        };
        let mut header16 = Header16::from(header16);
        let mut header32 = Header32::from(header32);
        header16.from_le();
        header32.from_le();

        // Determine various parameters.
        let sectors_per_fat = if bpb.sectors_per_fat_16 != 0 {
            bpb.sectors_per_fat_16 as u32
        } else {
            header32.sectors_per_fat_32
        };
        let legacy_root_size = (bpb.root_entry_count as u32 * 32 + bpb.bytes_per_sector as u32 - 1)
            / bpb.bytes_per_sector as u32;
        let data_sectors = sector_count
            - bpb.reserved_sector_count as u32
            - bpb.fat_count as u32 * sectors_per_fat
            - legacy_root_size;
        let cluster_count = data_sectors / bpb.sectors_per_cluster as u32;
        let sector_size_exp = bpb.bytes_per_sector.ilog2();
        let cluster_size_exp = bpb.sectors_per_cluster.ilog2() + sector_size_exp;
        let fat_sector = bpb.reserved_sector_count as u32;
        let legacy_root_sector = fat_sector + sectors_per_fat * bpb.fat_count as u32;
        let data_sector = legacy_root_sector + legacy_root_size;

        // Determine filesystem type.
        let fat_type = if cluster_count < 4085 {
            FatType::Fat12
        } else if cluster_count < 65525 {
            FatType::Fat16
        } else {
            FatType::Fat32
        };
        let root_dir_cluster = header32.first_root_cluster.wrapping_sub(2);

        // Prepare filesystem.
        let fs = FatFs {
            media,
            fat_type,
            allow_lfn: self.allow_lfn,
            cluster_size_exp,
            cluster_count,
            cluster_alloc: ClusterAlloc::new(cluster_count)?,
            data_offset: data_sector as u64 * bpb.bytes_per_sector as u64,
            sectors_per_fat,
            active_fat: header32.extra_flags as u8 & 15,
            mirror_fats: header32.extra_flags & 0x80 == 0,
            fat_sector,
            fat_count: bpb.fat_count,
            legacy_root_sector,
            legacy_root_size,
            fat12_mutex: Mutex::new(()),
            sector_size_exp,
            root_dir_cluster,
        };

        // Read the FAT for free clusters.
        for i in 0..cluster_count {
            if fs.fat_get(i)? == FatValue::Free {
                fs.cluster_alloc.free(i);
            }
        }

        Ok(Box::<dyn VfsOps>::from(Box::try_new(fs)?))
    }
}

fn register_fatfs() {
    FSDRIVERS
        .unintr_lock()
        .insert("vfat".into(), Box::new(FatFsDriver { allow_lfn: true }));
    FSDRIVERS
        .unintr_lock()
        .insert("msdos".into(), Box::new(FatFsDriver { allow_lfn: false }));
}

register_kmodule!(fatfs, [1, 0, 0], register_fatfs);
