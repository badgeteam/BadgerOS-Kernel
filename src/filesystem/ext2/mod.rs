// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    any::Any,
    mem::offset_of,
    num::{NonZeroU32, NonZeroU64},
    sync::atomic::{AtomicU32, Ordering},
};

use alloc::{
    boxed::Box,
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    sync::{Arc, Weak},
    vec::Vec,
};

use crate::{
    LogLevel,
    badgelib::time::Timespec,
    bindings::{
        device::HasBaseDevice,
        error::{EResult, Errno},
    },
    filesystem::{MakeFileSpec, NAME_MAX},
    kernel::sync::mutex::Mutex,
    process::usercopy::{UserSlice, UserSliceMut},
    util::MaybeMut,
};
use spec::*;

use super::{
    Dirent, FSDRIVERS, NodeType, Stat, UnlinkMode,
    media::Media,
    sysimpl::DentBuffer,
    vfs::{
        VNode, VNodeMtxInner, VNodeOps, Vfs, VfsDriver, VfsOps,
        mflags::{self, MFlags},
        vnflags,
    },
};

mod spec;

struct E2VNode {
    /// Inode number.
    ino: NonZeroU32,
    /// Cached inode structure.
    inode: Mutex<Inode>,
    /// On-disk inode structure offset.
    inode_offset: u64,
    /// Current size.
    size: u64,
    /// Node type.
    type_: NodeType,
}

impl E2VNode {
    fn group(&self, e2fs: &E2Fs) -> u32 {
        (u32::from(self.ino) - 1) / e2fs.inodes_per_group
    }

    /// Helper function to get one block ID from the inode.
    fn get_block(&self, e2fs: &E2Fs, block: u32, de_sparse: bool) -> EResult<Option<NonZeroU32>> {
        let guard = self.inode.lock_shared()?;
        let mut res = self.get_block_unlocked(e2fs, block, MaybeMut::Const(&guard))?;
        drop(guard);
        if de_sparse && res.is_none() {
            let mut guard = self.inode.lock()?;
            res = self.get_block_unlocked(e2fs, block, MaybeMut::Mut(&mut guard))?;
            debug_assert!(res.is_some());
        }
        Ok(res)
    }

    /// Helper function for finding singly-indirect blocks.
    fn get_indir3_block_impl(
        &self,
        e2fs: &E2Fs,
        index: (u32, u32, u32, u32),
        group_hint: u32,
        inode: &mut MaybeMut<'_, Inode>,
    ) -> EResult<Option<NonZeroU32>> {
        let ptr_block = match self.get_indir2_block_impl(
            e2fs,
            (index.1, index.2, index.3),
            group_hint,
            inode,
        )? {
            Some(x) => x,
            None => return Ok(None),
        };

        let ptr_block_diskoff = (u32::from(ptr_block) as u64) << e2fs.block_size_exp;
        let block = NonZeroU32::new(e2fs.media.read_le(ptr_block_diskoff + 4 * index.0 as u64)?);

        if let MaybeMut::Mut(inode) = inode
            && block.is_none()
        {
            let block = e2fs.alloc_block(group_hint)?;
            inode.realsize += 1 << (e2fs.block_size_exp - 9);

            e2fs.media
                .write_le(ptr_block_diskoff + 4 * index.0 as u64, u32::from(block))?;
            e2fs.media.write_le(
                self.inode_offset + offset_of!(Inode, realsize) as u64,
                inode.realsize,
            )?;

            Ok(Some(block))
        } else {
            Ok(block)
        }
    }

    /// Helper function for finding singly-indirect blocks.
    fn get_indir2_block_impl(
        &self,
        e2fs: &E2Fs,
        index: (u32, u32, u32),
        group_hint: u32,
        inode: &mut MaybeMut<'_, Inode>,
    ) -> EResult<Option<NonZeroU32>> {
        let ptr_block =
            match self.get_indir1_block_impl(e2fs, (index.1, index.2), group_hint, inode)? {
                Some(x) => x,
                None => return Ok(None),
            };

        let ptr_block_diskoff = (u32::from(ptr_block) as u64) << e2fs.block_size_exp;
        let block = NonZeroU32::new(e2fs.media.read_le(ptr_block_diskoff + 4 * index.0 as u64)?);

        if let MaybeMut::Mut(inode) = inode
            && block.is_none()
        {
            let block = e2fs.alloc_block(group_hint)?;
            inode.realsize += 1 << (e2fs.block_size_exp - 9);

            e2fs.media
                .write_le(ptr_block_diskoff + 4 * index.0 as u64, u32::from(block))?;
            e2fs.media.write_le(
                self.inode_offset + offset_of!(Inode, realsize) as u64,
                inode.realsize,
            )?;

            Ok(Some(block))
        } else {
            Ok(block)
        }
    }

    /// Helper function for finding singly-indirect blocks.
    fn get_indir1_block_impl(
        &self,
        e2fs: &E2Fs,
        index: (u32, u32),
        group_hint: u32,
        inode: &mut MaybeMut<'_, Inode>,
    ) -> EResult<Option<NonZeroU32>> {
        let ptr_block = match self.get_direct_block_impl(e2fs, index.1, group_hint, inode)? {
            Some(x) => x,
            None => return Ok(None),
        };

        let ptr_block_diskoff = (u32::from(ptr_block) as u64) << e2fs.block_size_exp;
        let block = NonZeroU32::new(e2fs.media.read_le(ptr_block_diskoff + 4 * index.0 as u64)?);

        if let MaybeMut::Mut(inode) = inode
            && block.is_none()
        {
            let block = e2fs.alloc_block(group_hint)?;
            inode.realsize += 1 << (e2fs.block_size_exp - 9);

            e2fs.media
                .write_le(ptr_block_diskoff + 4 * index.0 as u64, u32::from(block))?;
            e2fs.media.write_le(
                self.inode_offset + offset_of!(Inode, realsize) as u64,
                inode.realsize,
            )?;

            Ok(Some(block))
        } else {
            Ok(block)
        }
    }

    /// Helper function for finding direct blocks.
    fn get_direct_block_impl(
        &self,
        e2fs: &E2Fs,
        index: u32,
        group_hint: u32,
        inode: &mut MaybeMut<'_, Inode>,
    ) -> EResult<Option<NonZeroU32>> {
        let block = NonZeroU32::new(inode.data_blocks[index as usize]);

        if let MaybeMut::Mut(inode) = inode
            && block.is_none()
        {
            let block = e2fs.alloc_block(group_hint)?;
            inode.data_blocks[index as usize] = block.into();
            inode.realsize += 1 << (e2fs.block_size_exp - 9);

            e2fs.media.write_le(
                self.inode_offset + offset_of!(Inode, data_blocks) as u64 + index as u64 * 4,
                u32::from(block),
            )?;
            e2fs.media.write_le(
                self.inode_offset + offset_of!(Inode, realsize) as u64,
                inode.realsize,
            )?;

            Ok(Some(block))
        } else {
            Ok(block)
        }
    }

    /// Helper function to get one block ID from the inode.
    fn get_block_unlocked(
        &self,
        e2fs: &E2Fs,
        block: u32,
        mut inode: MaybeMut<'_, Inode>,
    ) -> EResult<Option<NonZeroU32>> {
        let group_hint = self.group(e2fs);

        let block_ptrs_exp = e2fs.block_size_exp - 2;
        let ind1_blocks = 1u32 << block_ptrs_exp;
        let ind2_blocks = 1u32 << (2 * block_ptrs_exp);
        let ind3_blocks = 1u32 << (3 * block_ptrs_exp);

        if block < 12 {
            // Direct blocks.
            self.get_direct_block_impl(e2fs, block, group_hint, &mut inode)
        } else if block - 12 < ind1_blocks {
            // Indirect blocks.
            let index = block - 12;
            self.get_indir1_block_impl(e2fs, (index, 12), group_hint, &mut inode)
        } else if block - 12 - ind1_blocks < ind2_blocks {
            // Doubly indirect blocks.
            let index = block - 12 - ind1_blocks;
            self.get_indir2_block_impl(
                e2fs,
                (index % ind1_blocks, index / ind1_blocks, 13),
                group_hint,
                &mut inode,
            )
        } else if block - 12 - ind1_blocks - ind2_blocks < ind3_blocks {
            // Triply indirect blocks.
            let index = block - 12 - ind1_blocks - ind2_blocks;
            self.get_indir3_block_impl(
                e2fs,
                (
                    index % ind1_blocks,
                    index / ind1_blocks % ind1_blocks,
                    index / ind2_blocks,
                    14,
                ),
                group_hint,
                &mut inode,
            )
        } else {
            unreachable!("Block index into inode too high");
        }
    }

    /// Recursive implementation of `remove_block`.
    fn remove_block_impl(
        &self,
        e2fs: &E2Fs,
        level: u8,
        fileblk: u32,
        block_ptr: NonZeroU32,
        inode: &mut Inode,
    ) -> EResult<bool> {
        if level == 0 {
            return Ok(true);
        }

        let index =
            (fileblk >> (e2fs.block_size_exp * level as u32)) % (1u32 << e2fs.block_size_exp);
        let block_ptr = (u32::from(block_ptr) as u64) << e2fs.block_size_exp;
        let mut tmp = [0u8; 4];
        e2fs.media.readk(block_ptr + 4 * index as u64, &mut tmp)?;

        let block = u32::from_le_bytes(tmp);
        if let Some(block) = NonZeroU32::new(block) {
            if self.remove_block_impl(e2fs, level - 1, fileblk, block, inode)? {
                // The entire block was freed, so free the pointer too.
                e2fs.free_block(block)?;
                e2fs.media.writek(block_ptr + 4 * index as u64, &[0u8; 4])?;
                inode.realsize -= 1u32 << (e2fs.block_size_exp - 9);
            }
        }

        // Check whether this indirection block is now empty.
        for i in 0..(1u32 << (e2fs.block_size_exp - 2)) {
            let mut tmp = [0u8; 4];
            e2fs.media.readk(block_ptr + i as u64 * 4, &mut tmp)?;
            if u32::from_le_bytes(tmp) != 0 {
                // There is still at least one block left.
                return Ok(false);
            }
        }

        // This block is now empty.
        Ok(true)
    }

    /// Helper function to set one block ID in the inode.
    fn remove_block_unlocked(&self, e2fs: &E2Fs, block: u32, inode: &mut Inode) -> EResult<()> {
        let block_size_exp = e2fs.block_size_exp;
        let block_ptrs_exp = block_size_exp - 2;
        let ind1_blocks = 1u32 << block_ptrs_exp;
        let ind2_blocks = 1u32 << (2 * block_ptrs_exp);
        let ind3_blocks = 1u32 << (3 * block_ptrs_exp);

        let index: usize;
        let level: u8;
        let fileblk: u32;

        if block < 12 {
            // Direct blocks.
            index = block as usize;
            fileblk = 0;
            level = 0;
        } else if block - 12 < ind1_blocks {
            // Indirect blocks.
            index = 12;
            fileblk = block - 12;
            level = 1;
        } else if block - 12 - ind1_blocks < ind2_blocks {
            // Doubly indirect blocks.
            index = 13;
            fileblk = block - 12 - ind1_blocks;
            level = 2;
        } else if block - 12 - ind1_blocks - ind2_blocks < ind3_blocks {
            // Triply indirect blocks.
            index = 13;
            fileblk = block - 12 - ind1_blocks - ind2_blocks;
            level = 3;
        } else {
            unreachable!("Block index into inode too high");
        }

        let block_ptr = NonZeroU32::new(inode.data_blocks[index]);
        let block_ptr = if let Some(x) = block_ptr {
            x
        } else {
            return Ok(());
        };
        if self.remove_block_impl(&e2fs, level, fileblk, block_ptr, inode)? {
            // The entire block was freed, so free the pointer too.
            e2fs.free_block(block_ptr)?;
            inode.data_blocks[index] = 0;
            inode.realsize -= 1u32 << (e2fs.block_size_exp - 9);
        }

        e2fs.media.writek(
            self.inode_offset + offset_of!(Inode, data_blocks) as u64 + index as u64 * 4,
            &[0u8; 4],
        )?;
        e2fs.media.writek(
            self.inode_offset + offset_of!(Inode, realsize) as u64,
            &inode.realsize.to_le_bytes(),
        )?;

        Ok(())
    }

    /// Helper function to iterate blocks within a certain range of this file.
    /// Callback args: file offset, disk offset, length.
    fn iter_blocks(
        &self,
        e2fs: &E2Fs,
        offset: u64,
        length: u64,
        de_sparse: bool,
        cb: &mut dyn FnMut(u64, Option<NonZeroU64>, u64) -> EResult<()>,
    ) -> EResult<()> {
        if length == 0 {
            return Ok(()); // Prevents underflow subtractions
        }

        // Convert the range into a range on disk.
        let block_size_exp = e2fs.block_size_exp;
        let fileblk_start = (offset >> block_size_exp) as u32;
        let fileblk_end = (offset + length).div_ceil(1u64 << block_size_exp) as u32;

        // Helper lambda for running the callback function.
        #[inline(always)]
        fn wrapper(
            block_size_exp: u32,
            offset: u64,
            length: u64,
            start: u32,
            end: u32,
            prev: Option<NonZeroU32>,
            cb: &mut dyn FnMut(u64, Option<NonZeroU64>, u64) -> EResult<()>,
        ) -> EResult<()> {
            let startblk = (start as u64) << block_size_exp;
            let endblk = (end as u64) << block_size_exp;
            let diskblk =
                prev.map(|x| ((u32::from(x) - (end - start - 1)) as u64) << block_size_exp);
            let start = startblk.max(offset);
            let end = endblk.min(offset + length);
            let disk = diskblk.map(|x| x + (start - startblk));
            let disk = disk.map(|x| unsafe { NonZeroU64::new_unchecked(x) });
            cb(start, disk, end - start)
        }

        // The start of the range of blocks found.
        let mut start = None::<u32>;
        let mut prev = None::<NonZeroU32>;

        for fileblk in fileblk_start..fileblk_end {
            let block = self.get_block(e2fs, fileblk, de_sparse)?;
            if de_sparse {
                debug_assert!(block.is_some());
            }
            if let Some(y) = start
                && prev.map(|x| unsafe { NonZeroU32::new_unchecked(u32::from(x) + 1) }) != block
            {
                wrapper(block_size_exp, offset, length, y, fileblk, prev, cb)?;
                start = Some(fileblk);
            } else if start.is_none() {
                start = Some(fileblk);
            }
            prev = block;
        }
        if let Some(y) = start {
            wrapper(block_size_exp, offset, length, y, fileblk_end, prev, cb)?;
        }

        Ok(())
    }

    /// Helper function to iterate over all dirents.
    /// Assumes the directory to use linked list dirents.
    fn iter_dirents(
        &self,
        vfs: &Vfs,
        cb: &mut dyn FnMut(&LinkedDent, u64, &[u8]) -> EResult<bool>,
    ) -> EResult<()> {
        let e2fs = vfs.get_ops_as::<E2Fs>();
        let block_size_exp = e2fs.block_size_exp;
        let mut offset = 0u64;
        let mut name = [0u8; NAME_MAX];
        while offset < self.size {
            // Read dirent header.
            let mut dent = [0u8; size_of::<LinkedDent>()];
            self.readk_impl(&e2fs, offset, &mut dent)?;
            let dent = LinkedDent::from(dent);
            let dent_end = offset.saturating_add(dent.record_len as u64);
            if dent.record_len % 4 != 0 || dent.record_len < size_of::<LinkedDent>() as u16 {
                logkf!(LogLevel::Error, "Dirent has invalid record length");
                vfs.check_eio_failed();
                break;
            } else if offset >> block_size_exp != (dent_end - 1) >> block_size_exp {
                logkf!(LogLevel::Error, "Dirent spans block boundary");
                vfs.check_eio_failed();
                break;
            } else if dent_end > self.size {
                logkf!(LogLevel::Error, "Dirent overflows end of directory");
                vfs.check_eio_failed();
                break;
            }

            // Read dirent name.
            let name_len = if e2fs.feature_incompat & feat::incompat::FILETYPE == 0 {
                // Rev. 0 uses what is later repurposed as file type for name length.
                dent.name_len as u16 + dent.file_type as u16 * 256
            } else {
                dent.name_len as u16
            };
            if name_len as usize > NAME_MAX {
                logkf!(LogLevel::Error, "File name too long");
                vfs.check_eio_failed();
            }
            self.readk_impl(&e2fs, offset + size_of::<LinkedDent>() as u64, &mut name)?;

            // Run dirent callback.
            if !cb(&dent, offset, &name[..name_len as usize])? {
                break;
            }

            // Go to next dirent.
            offset += dent.record_len as u64;
        }
        Ok(())
    }

    #[inline(always)]
    fn writek_impl(&self, e2fs: &E2Fs, offset: u64, wdata: &[u8]) -> EResult<()> {
        self.write_impl(e2fs, offset, UserSlice::new_kernel(wdata))
    }

    fn write_impl(&self, e2fs: &E2Fs, offset: u64, wdata: UserSlice<'_, u8>) -> EResult<()> {
        self.iter_blocks(
            e2fs,
            offset,
            wdata.len() as u64,
            true,
            &mut |fileoff, diskoff, len| try {
                e2fs.media.write(
                    diskoff.unwrap().into(),
                    wdata.subslice((fileoff - offset) as usize..(fileoff + len - offset) as usize),
                )?;
            },
        )
    }

    #[inline(always)]
    fn readk_impl(&self, e2fs: &E2Fs, offset: u64, rdata: &mut [u8]) -> EResult<()> {
        self.read_impl(e2fs, offset, UserSliceMut::new_kernel_mut(rdata))
    }

    fn read_impl(&self, e2fs: &E2Fs, offset: u64, mut rdata: UserSliceMut<'_, u8>) -> EResult<()> {
        self.iter_blocks(
            e2fs,
            offset,
            rdata.len() as u64,
            false,
            &mut |fileoff, diskoff, len| {
                if let Some(diskoff) = diskoff {
                    e2fs.media.read(
                        diskoff.into(),
                        rdata.subslice_mut(
                            (fileoff - offset) as usize..(fileoff + len - offset) as usize,
                        ),
                    )
                } else {
                    rdata
                        .subslice_mut(
                            (fileoff - offset) as usize..(fileoff - offset + len) as usize,
                        )
                        .fill(0)?;
                    Ok(())
                }
            },
        )
    }

    /// Allocate space for a new directory entry.
    fn alloc_dirent(&mut self, vnode_self: &VNode, length: u16) -> EResult<(u64, u16)> {
        let e2fs = vnode_self.vfs.get_ops_as();

        let mut res = None;
        self.iter_dirents(&vnode_self.vfs, &mut |dent, offset, _name| try {
            let min_record_len = dent.name_len.div_ceil(4) as u16 * 4 + 8;
            if dent.record_len - min_record_len >= length {
                self.writek_impl(
                    &e2fs,
                    offset + offset_of!(LinkedDent, record_len) as u64,
                    &min_record_len.to_le_bytes(),
                )?;
                res = Some((
                    offset + min_record_len as u64,
                    dent.record_len - min_record_len,
                ));
                false
            } else {
                true
            }
        })?;
        if let Some(res) = res {
            return Ok(res);
        }

        let pos = self.size;
        self.resize(vnode_self, pos + (1u64 << e2fs.block_size_exp))?;
        Ok((pos, 1u16 << e2fs.block_size_exp))
    }

    /// Create a new directory entry.
    fn create_dirent(
        &mut self,
        vnode_self: &VNode,
        ino: NonZeroU32,
        name: &[u8],
        type_: FileType,
    ) -> EResult<u64> {
        let e2fs = vnode_self.vfs.get_ops_as::<E2Fs>();
        if name.len() > 255 {
            return Err(Errno::ENAMETOOLONG);
        }
        let record_len = (8 + name.len().div_ceil(4) * 4) as u16;
        let (offset, record_len) = self.alloc_dirent(vnode_self, record_len)?;

        let dent = LinkedDent {
            ino: ino.into(),
            record_len,
            name_len: name.len() as u8,
            file_type: if e2fs.feature_incompat & feat::incompat::FILETYPE != 0 {
                type_ as u8
            } else {
                0 // NAME_MAX is 255 anyway.
            },
        };
        self.writek_impl(&e2fs, offset, &Into::<[u8; 8]>::into(dent))?;
        self.writek_impl(&e2fs, offset + 8, name)?;

        Ok(offset)
    }

    /// Delete a directory entry.
    fn delete_dirent(
        &mut self,
        vfs: &Vfs,
        name: &[u8],
        check_ino: Option<NonZeroU32>,
    ) -> EResult<()> {
        let e2fs = vfs.get_ops_as::<E2Fs>();

        // Find target dirent.
        let mut prev = None;
        let mut found = None;
        self.iter_dirents(vfs, &mut |dent, offset, dent_name| {
            if *name == *dent_name {
                found = Some((*dent, offset));
                Ok(false)
            } else {
                prev = Some((*dent, offset));
                Ok(true)
            }
        })?;
        let (found, offset) = found.ok_or(Errno::ENOENT)?;

        // The target dirent should have the same ino.
        if let Some(check_ino) = check_ino
            && found.ino != check_ino.into()
        {
            return Err(Errno::EIO);
        }

        if let Some((prev_dent, prev_offset)) = prev
            && prev_offset >> e2fs.block_size_exp == offset >> e2fs.block_size_exp
        {
            // Can merge with previous dirent.
            let record_len = prev_dent.record_len + found.record_len;
            self.writek_impl(
                &e2fs,
                prev_offset + offset_of!(LinkedDent, record_len) as u64,
                &record_len.to_le_bytes(),
            )?;
        } else {
            // Mark this dirent as unused.
            self.writek_impl(
                &e2fs,
                offset + offset_of!(LinkedDent, ino) as u64,
                &[0u8; 4],
            )?;
        }

        Ok(())
    }

    /// Helper function to recursively free all blocks owned by an inode.
    fn free_inode_blocks(e2fs: &E2Fs, data_blocks: [u32; 15]) -> EResult<()> {
        for i in 0..12 {
            if let Some(block) = NonZeroU32::new(data_blocks[i]) {
                e2fs.free_block(block)?;
            }
        }

        let ptr_per_block = 1u64 << (e2fs.block_size_exp - 2);

        // Singly-indirect blocks.
        if let Some(ind1_block) = NonZeroU32::new(data_blocks[13]) {
            for i in 0..ptr_per_block {
                let tmp = e2fs
                    .media
                    .read_le(((u32::from(ind1_block) as u64) << e2fs.block_size_exp) + i * 4)?;
                if let Some(block) = NonZeroU32::new(tmp) {
                    e2fs.free_block(block)?;
                }
            }
            e2fs.free_block(ind1_block)?;
        }

        // Doubly-indirect blocks.
        if let Some(ind2_block) = NonZeroU32::new(data_blocks[13]) {
            for i in 0..ptr_per_block {
                let tmp = e2fs
                    .media
                    .read_le(((u32::from(ind2_block) as u64) << e2fs.block_size_exp) + i * 4)?;
                if let Some(ind1_block) = NonZeroU32::new(tmp) {
                    for i in 0..ptr_per_block {
                        let tmp = e2fs.media.read_le(
                            ((u32::from(ind1_block) as u64) << e2fs.block_size_exp) + i * 4,
                        )?;
                        if let Some(block) = NonZeroU32::new(tmp) {
                            e2fs.free_block(block)?;
                        }
                    }
                    e2fs.free_block(ind1_block)?;
                }
            }
            e2fs.free_block(ind2_block)?;
        }

        // Triply-indirect blocks.
        if let Some(ind3_block) = NonZeroU32::new(data_blocks[13]) {
            for i in 0..ptr_per_block {
                let tmp = e2fs
                    .media
                    .read_le(((u32::from(ind3_block) as u64) << e2fs.block_size_exp) + i * 4)?;
                if let Some(ind2_block) = NonZeroU32::new(tmp) {
                    for i in 0..ptr_per_block {
                        let tmp = e2fs.media.read_le(
                            ((u32::from(ind2_block) as u64) << e2fs.block_size_exp) + i * 4,
                        )?;
                        if let Some(ind1_block) = NonZeroU32::new(tmp) {
                            for i in 0..ptr_per_block {
                                let tmp = e2fs.media.read_le(
                                    ((u32::from(ind1_block) as u64) << e2fs.block_size_exp) + i * 4,
                                )?;
                                if let Some(block) = NonZeroU32::new(tmp) {
                                    e2fs.free_block(block)?;
                                }
                            }
                            e2fs.free_block(ind1_block)?;
                        }
                    }
                    e2fs.free_block(ind2_block)?;
                }
            }
            e2fs.free_block(ind3_block)?;
        }

        Ok(())
    }

    /// Implementation of [`VNodeOps::unlink`] that doesn't check file type or empty dirs.
    fn unlink_impl(
        &mut self,
        vfs: &Vfs,
        name: &[u8],
        unlinked_ops: Option<&mut E2VNode>,
    ) -> EResult<()> {
        let e2fs = vfs.get_ops_as::<E2Fs>();

        // Delete the dirent.
        self.delete_dirent(vfs, name, unlinked_ops.as_ref().map(|x| x.ino))?;

        // Decrease inode refcount.
        let unlinked_ops = unlinked_ops.unwrap_or(self);
        let mut inode = unlinked_ops.inode.lock()?;
        inode.nlink = inode.nlink.checked_sub(1).ok_or(Errno::EIO)?;
        e2fs.media.write_le(
            unlinked_ops.inode_offset + offset_of!(Inode, nlink) as u64,
            inode.nlink,
        )?;

        Ok(())
    }
}

impl VNodeOps for E2VNode {
    fn write(&self, vnode_self: &VNode, offset: u64, wdata: UserSlice<'_, u8>) -> EResult<()> {
        let e2fs = vnode_self.vfs.get_ops_as::<E2Fs>();
        self.write_impl(&e2fs, offset, wdata)
    }

    fn read(&self, vnode_self: &VNode, offset: u64, rdata: UserSliceMut<'_, u8>) -> EResult<()> {
        let e2fs = vnode_self.vfs.get_ops_as::<E2Fs>();
        self.read_impl(&e2fs, offset, rdata)
    }

    fn resize(&mut self, vnode_self: &VNode, new_size: u64) -> EResult<()> {
        let e2fs = vnode_self.vfs.get_ops_as::<E2Fs>();
        let old_size = self.size;

        if new_size > old_size {
            // Erase new bytes.
            self.iter_blocks(
                &e2fs,
                old_size,
                new_size - old_size,
                false,
                &mut |_fileoff, diskoff, len| try {
                    if let Some(diskoff) = diskoff {
                        e2fs.media.write_zeroes(diskoff.into(), len)?;
                    }
                },
            )?;
        } else if new_size < old_size {
            // Garbage-collect blocks.
            let mut inode = self.inode.lock()?;
            let old_blocks = old_size.div_ceil(1u64 << e2fs.block_size_exp);
            let new_blocks = new_size.div_ceil(1u64 << e2fs.block_size_exp);
            for block in new_blocks..old_blocks {
                self.remove_block_unlocked(&e2fs, block as u32, &mut inode)?;
            }
        }

        // Update size in inode.
        let mut inode = self.inode.lock()?;
        inode.size = new_size as u32;
        e2fs.media.writek(
            self.inode_offset + offset_of!(Inode, size) as u64,
            &inode.size.to_le_bytes(),
        )?;
        if e2fs.feature_ro_compat & feat::compat_ro::LARGE_FILE != 0 {
            inode.dir_acl = (new_size >> 32) as u32;
            e2fs.media.writek(
                self.inode_offset + offset_of!(Inode, dir_acl) as u64,
                &inode.dir_acl.to_le_bytes(),
            )?;
        }

        self.size = new_size;
        Ok(())
    }

    fn find_dirent(&self, vnode_self: &VNode, name: &[u8]) -> EResult<Dirent> {
        let e2fs = vnode_self.vfs.get_ops_as::<E2Fs>();
        let mut res = Err(Errno::ENOENT);
        self.iter_dirents(&vnode_self.vfs, &mut |dent, off, dent_name| {
            if *name == *dent_name {
                try {
                    let type_ = e2fs.get_inode_type(dent)?;
                    res = Ok(Dirent {
                        ino: dent.ino as u64,
                        type_,
                        name: dent_name.into(),
                        dirent_disk_off: 0,
                        dirent_off: off,
                    });
                    false
                }
            } else {
                Ok(true)
            }
        })?;
        res
    }

    fn get_dirents(
        &self,
        vnode_self: &VNode,
        mut offset: u64,
        buffer: &mut DentBuffer<'_>,
    ) -> EResult<u64> {
        let e2fs = vnode_self.vfs.get_ops_as::<E2Fs>();
        self.iter_dirents(&vnode_self.vfs, &mut |dent, off, name| {
            if off < offset {
                Ok(true)
            } else {
                let type_ = e2fs.get_inode_type(dent)?;
                let fits = buffer.push(Dirent {
                    ino: dent.ino as _,
                    type_,
                    name: name.into(),
                    dirent_disk_off: 0,
                    dirent_off: off,
                })?;
                if fits {
                    offset = off + dent.record_len as u64;
                }
                Ok(fits)
            }
        })?;
        Ok(offset)
    }

    fn unlink(
        &mut self,
        vnode_self: &VNode,
        name: &[u8],
        mode: UnlinkMode,
        unlinked_vnode: Option<Arc<VNode>>,
    ) -> EResult<()> {
        let e2fs = vnode_self.vfs.get_ops_as::<E2Fs>();

        // Get E2VNode from unlinked_vnode, or open temporarily if not present.
        // This will be used to decrease the links count later.
        let mut unlinked_guard = match unlinked_vnode.as_ref() {
            Some(x) => Some(x.mtx.lock()?),
            None => None,
        };
        let unlinked_ops = unlinked_guard.as_mut().map(|x| &mut x.ops);
        let mut tmp_ops = if unlinked_ops.is_none() {
            let dent = self.find_dirent(vnode_self, name)?;
            Some(e2fs.open_impl(unsafe { NonZeroU32::new_unchecked(dent.ino as u32) })?)
        } else {
            None
        };
        let unlinked_ops = (unlinked_ops
            .unwrap_or_else(|| tmp_ops.as_mut().unwrap())
            .as_mut() as &mut dyn Any)
            .downcast_mut::<E2VNode>()
            .unwrap();

        let is_dir = unlinked_ops.type_ == NodeType::Directory;
        if is_dir {
            if mode == UnlinkMode::FileOnly {
                return Err(Errno::EISDIR);
            }
            // Unlinked directories must be empty.
            unlinked_ops.iter_dirents(&vnode_self.vfs, &mut |_dent, _off, name| {
                if *name == *b"." || *name == *b".." {
                    Ok(true)
                } else {
                    Err(Errno::ENOTEMPTY)
                }
            })?;

            // Remove `.` and `..` because `unlink_impl` doesn't do this automatically.
            unlinked_ops.unlink_impl(&vnode_self.vfs, b".", None)?;
            unlinked_ops.unlink_impl(&vnode_self.vfs, b"..", Some(self))?;
            debug_assert!(unlinked_ops.inode.unintr_lock_shared().nlink == 1);
        } else if mode == UnlinkMode::DirOnly {
            return Err(Errno::ENOTDIR);
        }

        // Inode is now ready to be unlinked.
        self.unlink_impl(&vnode_self.vfs, name, Some(unlinked_ops))?;

        // Save nlink here so the borrow of unlinked_guard/tmp_ops via unlinked_ops ends.
        let nlink = unlinked_ops.inode.lock_shared()?.nlink;

        if nlink == 0 {
            match unlinked_guard.as_mut() {
                Some(guard) => {
                    // VNode is still open; mark it so no further modifications are allowed.
                    guard.flags |= vnflags::REMOVED;
                }
                None => {
                    let e2vnode = (tmp_ops.as_mut().unwrap().as_mut() as &mut dyn Any)
                        .downcast_mut::<E2VNode>()
                        .unwrap();
                    Self::free_inode_blocks(&e2fs, e2vnode.inode.lock_shared()?.data_blocks)?;
                    e2fs.free_inode(e2vnode.ino)?;
                }
            }
        }

        Ok(())
    }

    fn link(&mut self, vnode_self: &VNode, name: &[u8], vnode: &VNode) -> EResult<()> {
        let e2fs = vnode_self.vfs.get_ops_as::<E2Fs>();

        // Increase the inode's links count.
        let vnode_ops = vnode.get_ops_as::<E2VNode>();
        let mut inode = vnode_ops.inode.lock()?;
        inode.nlink = inode.nlink.checked_add(1).ok_or(Errno::EMLINK)?;
        e2fs.media.write_le(
            vnode_ops.inode_offset + offset_of!(Inode, nlink) as u64,
            inode.nlink,
        )?;

        // Create a new dirent for the inode.
        self.create_dirent(
            vnode_self,
            NonZeroU32::new(vnode.ino as u32).unwrap(),
            name,
            vnode.type_.into(),
        )?;
        Ok(())
    }

    fn make_file(
        &mut self,
        vnode_self: &VNode,
        name: &[u8],
        spec: MakeFileSpec,
    ) -> EResult<(Dirent, Box<dyn VNodeOps>)> {
        // Allocate an empty inode.
        let e2fs = vnode_self.vfs.get_ops_as::<E2Fs>();
        let (ino, inode_offset) = e2fs.alloc_inode(self.group(&e2fs))?;

        // Create empty inode.
        let mode = Mode::try_from(spec.node_type())?;
        let now = Timespec::now().sec as u32;
        let inode = Inode {
            mode: mode as u16 | 0o777,
            atime: now,
            ctime: now,
            mtime: now,
            nlink: 1,
            ..Default::default()
        };
        let mut ops = Box::try_new(E2VNode {
            ino,
            inode: Mutex::new(inode),
            inode_offset,
            size: 0,
            type_: spec.node_type(),
        })?;

        let mut dirent_off = 0;
        let res: EResult<()> = try {
            // Try to fill in the inode data.
            match &spec {
                MakeFileSpec::Directory => {
                    // Create . and .. entries.
                    if self.inode.lock_shared()?.nlink == u16::MAX {
                        return Err(Errno::EMLINK);
                    }

                    {
                        let mut inode = ops.inode.lock()?;
                        inode.nlink = 2;
                        ops.size = 1u64 << e2fs.block_size_exp;
                        inode.size = ops.size as u32;
                    }

                    let dent = LinkedDent {
                        ino: ino.into(),
                        record_len: 12,
                        name_len: 1,
                        file_type: FileType::Directory as u8,
                    };
                    ops.writek_impl(&e2fs, 0, &Into::<[u8; 8]>::into(dent))?;
                    ops.writek_impl(&e2fs, 8, b".\0\0\0")?;

                    let dent = LinkedDent {
                        ino: self.ino.into(),
                        record_len: (1u16 << e2fs.block_size_exp) - 12,
                        name_len: 2,
                        file_type: FileType::Directory as u8,
                    };
                    ops.writek_impl(&e2fs, 12, &Into::<[u8; 8]>::into(dent))?;
                    ops.writek_impl(&e2fs, 20, b"..\0\0")?;

                    // Update link counts.
                    {
                        let mut inode = self.inode.lock()?;
                        inode.nlink += 1;
                        e2fs.media.write_le(
                            self.inode_offset + offset_of!(Inode, nlink) as u64,
                            inode.nlink,
                        )?;
                    }

                    // Bump block group dirs count.
                    {
                        let group =
                            e2fs.get_block_group((u32::from(ino) - 1) / e2fs.inodes_per_group)?;
                        let mut desc = group.desc.lock()?;
                        desc.used_dirs_count =
                            desc.used_dirs_count.checked_add(1).ok_or(Errno::EIO)?;
                        e2fs.media.write_le(
                            group.disk_offset + offset_of!(BlockGroupDesc, used_dirs_count) as u64,
                            desc.used_dirs_count,
                        )?;
                    }
                }
                MakeFileSpec::Symlink(value) => {
                    ops.size = value.len() as u64;
                    ops.inode.lock()?.size = ops.size as u32;
                    if value.len() < 60 {
                        // Short symlinks stored directly in inode buffer.
                        let mut buffer = [0u8; 60];
                        buffer[..value.len()].copy_from_slice(value);
                        let mut inode = ops.inode.lock()?;
                        inode.data_blocks = unsafe { core::mem::transmute(buffer) };
                        for i in 0..15 {
                            inode.data_blocks[i] = u32::from_le(inode.data_blocks[i]);
                        }
                    } else {
                        // Long symlinks stored in blocks.
                        ops.inode.lock()?.realsize = 1u32 << (e2fs.block_size_exp - 9);
                        ops.writek_impl(&e2fs, 0, value)?;
                    }
                }
                _ => (),
            }

            // Create the dirent for this new file.
            dirent_off = self.create_dirent(vnode_self, ino, name, spec.node_type().into())?;
        };
        if let Err(x) = res {
            let _ = e2fs.free_inode(ino);
            return Err(x);
        }

        // Write the base inode struct.
        e2fs.media.writek(
            inode_offset,
            &Into::<[u8; _]>::into(*ops.inode.lock_shared()?),
        )?;
        // Fill the space not covered by our inode struct with zeroes.
        e2fs.media.write_zeroes(
            inode_offset + size_of::<Inode>() as u64,
            e2fs.inode_size as u64 - size_of::<Inode>() as u64,
        )?;

        // Create dummy dirent.
        let dirent = Dirent {
            ino: u32::from(ino) as u64,
            type_: spec.node_type(),
            name: name.into(),
            dirent_disk_off: 0,
            dirent_off,
        };

        Ok((dirent, ops))
    }

    fn rename(&mut self, vnode_self: &VNode, old_name: &[u8], new_name: &[u8]) -> EResult<Dirent> {
        // Find source dirent.
        let mut found = None;
        self.iter_dirents(&vnode_self.vfs, &mut |dent, offset, dent_name| {
            if *old_name == *dent_name {
                found = Some((*dent, offset));
                Ok(false)
            } else {
                Ok(true)
            }
        })?;
        let (found, _offset) = found.ok_or(Errno::ENOENT)?;

        // Create new dirent.
        self.create_dirent(
            vnode_self,
            NonZeroU32::new(found.ino).ok_or(Errno::EIO)?,
            new_name,
            found.file_type.try_into()?,
        )?;

        // Delete old dirent.
        self.delete_dirent(
            &vnode_self.vfs,
            old_name,
            Some(NonZeroU32::new(found.ino).unwrap()),
        )?;

        Ok(Dirent {
            ino: found.ino as u64,
            type_: FileType::try_from(found.file_type)?.into(),
            name: new_name.into(),
            dirent_disk_off: 0,
            dirent_off: 0,
        })
    }

    fn readlink(&self, vnode_self: &VNode) -> EResult<Box<[u8]>> {
        if self.size > NAME_MAX as u64 {
            return Err(Errno::ENAMETOOLONG);
        }
        let mut link = Vec::try_with_capacity(self.size as usize)?;
        link.resize(self.size as usize, 0);
        if self.size <= 60 {
            let data_blocks = &self.inode.lock_shared()?.data_blocks;
            let mut block_bytes = [0u8; 60];
            for i in 0..15 {
                block_bytes[i * 4..i * 4 + 4].copy_from_slice(&data_blocks[i].to_le_bytes());
            }
            link.copy_from_slice(&block_bytes[..self.size as usize]);
        } else {
            self.read(vnode_self, 0, UserSliceMut::new_kernel_mut(&mut link))?;
        }
        Ok(link.into())
    }

    fn stat(&self, vnode_self: &VNode) -> EResult<Stat> {
        let e2fs = vnode_self.vfs.get_ops_as::<E2Fs>();
        let inode = self.inode.lock_shared()?;
        Ok(Stat {
            dev: e2fs
                .media
                .device()
                .as_ref()
                .map(|dev| ((u32::from(dev.id()) as u64) << 32) | dev.class() as u64)
                .unwrap_or(0),
            ino: u32::from(self.ino) as u64,
            mode: inode.mode,
            nlink: inode.nlink,
            uid: inode.uid,
            gid: inode.gid,
            rdev: 0,
            size: self.size,
            blksize: 1u64 << e2fs.block_size_exp,
            blocks: inode.realsize as u64,
            atim: Timespec {
                sec: inode.atime as u64,
                nsec: 0,
            },
            mtim: Timespec {
                sec: inode.mtime as u64,
                nsec: 0,
            },
            ctim: Timespec {
                sec: inode.ctime as u64,
                nsec: 0,
            },
        })
    }

    fn get_inode(&self) -> u64 {
        u32::from(self.ino) as u64
    }

    fn get_size(&self, _vnode_self: &VNode) -> u64 {
        self.size
    }

    fn get_type(&self, _vnode_self: &VNode) -> NodeType {
        self.type_
    }

    fn sync(&self, vnode_self: &VNode) -> EResult<()> {
        let e2fs = vnode_self.vfs.get_ops_as::<E2Fs>();
        e2fs.media
            .sync(self.inode_offset, size_of::<Inode>() as u64)?;
        self.iter_blocks(&e2fs, 0, self.size, false, &mut |_fileoff, diskoff, len| {
            if let Some(diskoff) = diskoff {
                e2fs.media.sync(diskoff.into(), len)
            } else {
                Ok(())
            }
        })
    }

    fn close(&mut self, vnode_self: &VNode) {
        let e2fs = vnode_self.vfs.get_ops_as::<E2Fs>();
        let inode = self.inode.unintr_lock();
        if inode.nlink != 0
            || (vnode_self.vfs.flags.load(Ordering::Relaxed) & mflags::READ_ONLY) != 0
        {
            // Inode was closed, but isn't unlinked.
            return;
        }

        // Unlinked inode was closed; free blocks.
        let _ = vnode_self.vfs.check_eio(
            try {
                Self::free_inode_blocks(&vnode_self.vfs.get_ops_as(), inode.data_blocks)?;
                e2fs.free_inode(self.ino)?;
            },
        );
    }
}

struct E2BlockGroup {
    /// Block group index.
    index: u32,
    /// On-disk offset of block group descriptor entry.
    disk_offset: u64,
    /// Cached block group descriptor entry.
    desc: Mutex<BlockGroupDesc>,
}

struct E2Fs {
    /// ID of the first data block.
    first_data_block: u32,
    /// How many inodes are associated with each block group.
    inodes_per_group: u32,
    /// How many blocks are associated with each block group.
    blocks_per_group: u32,
    /// How many bytes large an inode table entry is.
    inode_size: u16,
    /// Enabled COMPAT features.
    feature_compat: u32,
    /// Enabled INCOMPAT features.
    feature_incompat: u32,
    /// Enabled RO_COMPAT features.
    feature_ro_compat: u32,
    /// Log-base 2 of block size.
    block_size_exp: u32,
    /// Number of block groups.
    block_groups: u32,
    /// Offset of the block group descriptor table.
    bgdt_offset: u64,
    /// Number of available blocks.
    free_blocks: AtomicU32,
    /// Number of available inodes.
    free_inodes: AtomicU32,
    /// Media that the filesystem is mounted on.
    media: Media,
    /// Cached block group descriptors.
    block_group_desc: Mutex<BTreeMap<u32, Weak<E2BlockGroup>>>,
    /// Which block group descriptors need to be copied to backups on sync.
    dirty_bgdt_ents: Mutex<BTreeSet<u32>>,
}

impl E2Fs {
    /// Try to allocate a block, starting in block group `group`.
    fn alloc_block(&self, mut group_hint: u32) -> EResult<NonZeroU32> {
        // Reserve one block.
        self.free_blocks
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                old.checked_sub(1)
            })
            .map_err(|_| Errno::ENOSPC)?;

        loop {
            let group = self.get_block_group(group_hint)?;
            let mut guard = group.desc.lock()?;

            if guard.free_block_count > 0 {
                for i in 0..self.blocks_per_group.div_ceil(size_of::<usize>() as u32) {
                    // Offset of the part of the bitmap being tested.
                    let offset = ((guard.block_bitmap as u64) << self.block_size_exp)
                        + i as u64 * size_of::<usize>() as u64;

                    // Fetch bytes from the bitmap.
                    let mut tmp = [0u8; size_of::<usize>()];
                    self.media.readk(offset, &mut tmp)?;
                    let mut tmp = usize::from_le_bytes(tmp);

                    if tmp != usize::MAX {
                        // Mark block as used.
                        let bitpos = tmp.trailing_ones();
                        tmp |= 1usize << bitpos;
                        self.media.writek(offset, &tmp.to_le_bytes())?;

                        // Update BGDT on disk.
                        guard.free_block_count -= 1;
                        self.media.writek(
                            group.disk_offset + offset_of!(BlockGroupDesc, free_block_count) as u64,
                            &guard.free_block_count.to_le_bytes(),
                        )?;
                        // Mark backup BGDT entry out of date.
                        self.dirty_bgdt_ents.unintr_lock().insert(group_hint);

                        // Return the newly allocated block.
                        return Ok(NonZeroU32::new(
                            bitpos
                                + i as u32 * usize::BITS
                                + group_hint * self.blocks_per_group
                                + self.first_data_block,
                        )
                        .unwrap());
                    }
                }

                // There should have been a free block by now.
                return Err(Errno::EIO);
            }

            group_hint = (1 + group_hint) % self.block_groups;
        }
    }

    /// Mark a block as free.
    fn free_block(&self, block: NonZeroU32) -> EResult<()> {
        let block = u32::from(block);
        let group =
            block.checked_sub(self.first_data_block).ok_or(Errno::EIO)? / self.blocks_per_group;
        let index = (block - self.first_data_block) % self.blocks_per_group;

        let block_group = self.get_block_group(group)?;
        let mut guard = block_group.desc.lock()?;
        let bitmap_off = ((guard.block_bitmap as u64) << self.block_size_exp)
            + size_of::<usize>() as u64 * (index / usize::BITS) as u64;

        // Clear block from the bitmap.
        let mut tmp = self.media.read_le::<usize>(bitmap_off)?;
        if (tmp >> (index % usize::BITS)) & 1 == 0 {
            logkf!(LogLevel::Error, "Block {} marked as free twice", block);
            return Err(Errno::EIO);
        }
        tmp &= !(1usize << (index % usize::BITS));
        self.media.write_le(bitmap_off, tmp)?;

        // Update block group free block count.
        guard.free_block_count = guard.free_block_count.checked_add(1).ok_or(Errno::EIO)?;
        self.media.write_le(
            block_group.disk_offset + offset_of!(BlockGroupDesc, free_block_count) as u64,
            guard.free_block_count,
        )?;
        // Mark backup BGDT entry out of date.
        self.dirty_bgdt_ents.unintr_lock().insert(group);

        self.free_blocks.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Allocate a new uninitialized inode.
    fn alloc_inode(&self, mut group_hint: u32) -> EResult<(NonZeroU32, u64)> {
        // Reserve one block.
        self.free_inodes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                old.checked_sub(1)
            })
            .map_err(|_| Errno::ENOSPC)?;

        loop {
            let group = self.get_block_group(group_hint)?;
            let mut guard = group.desc.lock()?;

            if guard.free_inode_count > 0 {
                for i in 0..self.inodes_per_group.div_ceil(size_of::<usize>() as u32) {
                    // Offset of the part of the bitmap being tested.
                    let offset = ((guard.inode_bitmap as u64) << self.block_size_exp)
                        + i as u64 * size_of::<usize>() as u64;

                    // Fetch bytes from the bitmap.
                    let mut tmp = [0u8; size_of::<usize>()];
                    self.media.readk(offset, &mut tmp)?;
                    let mut tmp = usize::from_le_bytes(tmp);

                    if tmp != usize::MAX {
                        // Mark inode as used.
                        let bitpos = tmp.trailing_ones();
                        tmp |= 1usize << bitpos;
                        self.media.writek(offset, &tmp.to_le_bytes())?;

                        // Update BGDT on disk.
                        guard.free_inode_count -= 1;
                        self.media.writek(
                            group.disk_offset + offset_of!(BlockGroupDesc, free_inode_count) as u64,
                            &guard.free_inode_count.to_le_bytes(),
                        )?;
                        // Mark backup BGDT entry out of date.
                        self.dirty_bgdt_ents.unintr_lock().insert(group_hint);

                        // Return the newly allocated inode number.
                        let local_ino = bitpos + i as u32 * usize::BITS;
                        let ino = local_ino + group_hint * self.inodes_per_group + 1;
                        return Ok((
                            NonZeroU32::new(ino).unwrap(),
                            ((guard.inode_table as u64) << self.block_size_exp)
                                + local_ino as u64 * self.inode_size as u64,
                        ));
                    }
                }

                // There should have been a free inode by now.
                return Err(Errno::EIO);
            }

            group_hint = (1 + group_hint) % self.block_groups;
        }
    }

    /// Mark an inode as free; writes 0 to nlink on disk and marks as free in the bitmap.
    fn free_inode(&self, ino: NonZeroU32) -> EResult<()> {
        let ino = u32::from(ino);
        let group = (ino - 1) / self.inodes_per_group;
        let index = (ino - 1) % self.inodes_per_group;

        let block_group = self.get_block_group(group)?;
        let mut guard = block_group.desc.unintr_lock();
        let bitmap_off = ((guard.inode_bitmap as u64) << self.block_size_exp)
            + size_of::<usize>() as u64 * (index / usize::BITS) as u64;
        let inode_offset = ((guard.inode_table as u64) << self.block_size_exp)
            + index as u64 * self.inode_size as u64;

        // Clear inode from the bitmap.
        let mut tmp = self.media.read_le::<usize>(bitmap_off)?;
        if (tmp >> (index % usize::BITS)) & 1 == 0 {
            logkf!(LogLevel::Error, "Inode {} marked as free twice", ino);
            return Err(Errno::EIO);
        }
        tmp &= !(1usize << (index % usize::BITS));
        self.media.write_le(bitmap_off, tmp)?;

        // Update inode group free inode count.
        guard.free_inode_count = guard.free_inode_count.checked_add(1).ok_or(Errno::EIO)?;
        self.media.write_le(
            block_group.disk_offset + offset_of!(BlockGroupDesc, free_inode_count) as u64,
            guard.free_inode_count,
        )?;
        if Mode::try_from(
            self.media
                .read_le::<u16>(inode_offset + offset_of!(Inode, mode) as u64)?,
        )? == Mode::Directory
        {
            guard.used_dirs_count = guard.used_dirs_count.checked_sub(1).ok_or(Errno::EIO)?;
            self.media.write_le(
                block_group.disk_offset + offset_of!(BlockGroupDesc, used_dirs_count) as u64,
                guard.used_dirs_count,
            )?;
        }
        // Mark backup BGDT entry out of date.
        self.dirty_bgdt_ents.unintr_lock().insert(group);

        // Clear inode to zeroes.
        let _ = self
            .media
            .write_zeroes(inode_offset, self.inode_size as u64);
        // Set inode dtime.
        let _ = self.media.write_le(
            inode_offset + offset_of!(Inode, dtime) as u64,
            Timespec::now().sec as u32,
        );

        self.free_inodes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Helper function to get the inode type for a dirent.
    /// Will query the actual inode for ext2 rev. 0.
    fn get_inode_type(&self, dent: &LinkedDent) -> EResult<NodeType> {
        if self.feature_incompat & feat::incompat::FILETYPE == 0 {
            let group = dent.ino.checked_sub(1).ok_or(Errno::EIO)? / self.inodes_per_group;
            let index = (dent.ino - 1) % self.inodes_per_group;
            let block_group = self.get_block_group(group)?;
            let inode_offset = ((block_group.desc.unintr_lock_shared().inode_table as u64)
                << self.block_size_exp)
                + self.inode_size as u64 * index as u64;
            let mode: u16 = self
                .media
                .read_le(inode_offset + offset_of!(Inode, mode) as u64)?;
            Ok(Mode::try_from(mode)?.into())
        } else {
            Ok(FileType::try_from(dent.file_type)?.into())
        }
    }

    /// Get block group descriptor by block group index.
    fn get_block_group(&self, index: u32) -> EResult<Arc<E2BlockGroup>> {
        if let Some(res) = try {
            self.block_group_desc
                .unintr_lock_shared()
                .get(&index)?
                .upgrade()?
        } {
            return Ok(res.clone());
        }

        let mut guard = self.block_group_desc.unintr_lock();
        if let Some(res) = try { guard.get(&index)?.upgrade()? } {
            return Ok(res.clone()); // Race condition: Entry created by another thread.
        }

        let mut group_desc = [0u8; size_of::<BlockGroupDesc>()];
        let disk_offset = self.bgdt_offset + index as u64 * size_of::<BlockGroupDesc>() as u64;
        self.media.readk(disk_offset, &mut group_desc)?;
        let group_desc = BlockGroupDesc::from(group_desc);

        let ent = Arc::try_new(E2BlockGroup {
            disk_offset,
            index,
            desc: Mutex::new(group_desc),
        })?;
        guard.insert(index, Arc::downgrade(&ent));

        Ok(ent)
    }

    /// Implementation of [`Self::open_root`] and [`Self::open`].
    fn open_impl(&self, ino: NonZeroU32) -> EResult<Box<dyn VNodeOps>> {
        let block_group = (u32::from(ino) - 1) / self.inodes_per_group;
        let index = (u32::from(ino) - 1) % self.inodes_per_group;

        // Load the inode structure from disk.
        let block_group = self.get_block_group(block_group)?;
        let inode_offset = ((block_group.desc.lock_shared()?.inode_table as u64)
            << self.block_size_exp)
            + index as u64 * self.inode_size as u64;
        let mut inode = [0u8; size_of::<Inode>()];
        self.media.readk(inode_offset, &mut inode)?;
        let inode = Inode::from(inode);

        // Inode size field depends on Ext2 revision and whether it's a directory.
        let size = if self.feature_incompat & feat::incompat::FILETYPE == 0
            || inode.mode & 0xf000 == Mode::Directory as u16
        {
            inode.size as u64
        } else {
            inode.size as u64 | ((inode.dir_acl as u64) << 32)
        };
        let mode = Mode::try_from(inode.mode)?;
        let type_: NodeType = mode.into();

        // Loaded successfully, make VNode.
        Ok(Box::<dyn VNodeOps>::from(Box::try_new(E2VNode {
            ino,
            inode: Mutex::new(inode),
            inode_offset,
            size,
            type_,
        })?))
    }

    /// Helper function to iterate all superblock backups by block group number.
    fn iter_superblocks(
        &self,
        include_orig: bool,
        cb: &mut dyn FnMut(u64, u64) -> EResult<()>,
    ) -> EResult<()> {
        if include_orig {
            cb(1024, self.bgdt_offset)?;
        }
        if self.feature_ro_compat & feat::compat_ro::SPARSE_SUPER != 0 {
            let mut i = 3;
            while i < self.block_groups {
                let offset = (i as u64 * self.blocks_per_group as u64) << self.block_size_exp;
                cb(offset, offset + (1u64 << self.block_size_exp))?;
                i *= 3;
            }
            let mut i = 5;
            while i < self.block_groups {
                let offset = (i as u64 * self.blocks_per_group as u64) << self.block_size_exp;
                cb(offset, offset + (1u64 << self.block_size_exp))?;
                i *= 5;
            }
            let mut i = 7;
            while i < self.block_groups {
                let offset = (i as u64 * self.blocks_per_group as u64) << self.block_size_exp;
                cb(offset, offset + (1u64 << self.block_size_exp))?;
                i *= 7;
            }
        } else {
            for i in 1..self.block_groups {
                let offset = (i as u64 * self.blocks_per_group as u64) << self.block_size_exp;
                cb(offset, offset + (1u64 << self.block_size_exp))?;
            }
        }
        Ok(())
    }
}

impl VfsOps for E2Fs {
    fn media(&self) -> Option<&Media> {
        Some(&self.media)
    }

    fn uses_inodes(&self) -> bool {
        true
    }

    fn read_only(&self) -> bool {
        self.feature_ro_compat & !RO_COMPAT_SUPPORTED != 0
    }

    fn block_size_exp(&self) -> u8 {
        self.block_size_exp as u8
    }

    fn open_root(&self, _vnode_self: &Arc<Vfs>) -> EResult<Box<dyn VNodeOps>> {
        self.open_impl(NonZeroU32::new(ROOT_INO).unwrap())
    }

    fn open(&self, _vnode_self: &Arc<Vfs>, dirent: &Dirent) -> EResult<Box<dyn VNodeOps>> {
        self.open_impl(NonZeroU32::new(dirent.ino as u32).ok_or(Errno::EIO)?)
    }

    fn rename(
        &self,
        vnode_self: &Arc<Vfs>,
        _src_dir: &Arc<VNode>,
        src_name: &[u8],
        src_mutexinner: &mut VNodeMtxInner,
        dest_dir: &Arc<VNode>,
        dest_name: &[u8],
        dest_mutexinner: &mut VNodeMtxInner,
    ) -> EResult<Dirent> {
        let src_ops = (src_mutexinner.ops.as_mut() as &mut dyn Any)
            .downcast_mut::<E2VNode>()
            .unwrap();
        let dest_ops = (dest_mutexinner.ops.as_mut() as &mut dyn Any)
            .downcast_mut::<E2VNode>()
            .unwrap();

        // Find source dirent.
        let mut found = None;
        src_ops.iter_dirents(&vnode_self, &mut |dent, offset, dent_name| {
            if *src_name == *dent_name {
                found = Some((*dent, offset));
                Ok(false)
            } else {
                Ok(true)
            }
        })?;
        let (found, _offset) = found.ok_or(Errno::ENOENT)?;

        // Create new dirent.
        dest_ops.create_dirent(
            dest_dir,
            NonZeroU32::new(found.ino).ok_or(Errno::EIO)?,
            dest_name,
            found.file_type.try_into()?,
        )?;

        // Delete old dirent.
        src_ops.delete_dirent(
            &vnode_self,
            src_name,
            Some(NonZeroU32::new(found.ino).unwrap()),
        )?;

        Ok(Dirent {
            ino: found.ino as u64,
            type_: FileType::try_from(found.file_type)?.into(),
            name: dest_name.into(),
            dirent_disk_off: 0,
            dirent_off: 0,
        })
    }

    fn sync(&self) -> EResult<()> {
        let free_blocks = self.free_blocks.load(Ordering::Relaxed);
        let free_inodes = self.free_inodes.load(Ordering::Relaxed);

        self.iter_superblocks(true, &mut |superblock, bgdt| try {
            self.media.writek(
                superblock + offset_of!(Superblock, free_block_count) as u64,
                &free_blocks.to_le_bytes(),
            )?;
            self.media.writek(
                superblock + offset_of!(Superblock, free_inode_count) as u64,
                &free_inodes.to_le_bytes(),
            )?;
            self.media
                .sync(superblock, size_of::<Superblock>() as u64)?;
            self.media.sync(
                bgdt,
                self.block_groups as u64 * size_of::<BlockGroupDesc>() as u64,
            )?;
        })
    }
}

/// What EXT2 compat features are supported.
pub const COMPAT_SUPPORTED: u32 = 0;
/// What EXT2 incompat features are supported.
pub const INCOMPAT_SUPPORTED: u32 = feat::incompat::FILETYPE;
/// What EXT2 ro-compat features are supported.
pub const RO_COMPAT_SUPPORTED: u32 = feat::compat_ro::LARGE_FILE | feat::compat_ro::SPARSE_SUPER;

struct E2FsDriver {}

impl VfsDriver for E2FsDriver {
    fn detect(&self, media: &Media) -> EResult<bool> {
        // Load the superblock.
        let mut superblock = [0u8; size_of::<Superblock>()];
        media.readk(1024, &mut superblock)?;
        let superblock = Superblock::from(superblock);

        // Check the magic value.
        Ok(superblock.magic == MAGIC)
    }

    fn mount(&self, media: Option<Media>, _mflags: MFlags) -> EResult<Box<dyn VfsOps>> {
        let media = media.ok_or(Errno::ENODEV)?;

        // Load the superblock.
        let mut superblock = [0u8; size_of::<Superblock>()];
        media.readk(1024, &mut superblock)?;
        let superblock = Superblock::from(superblock);
        let block_size_exp = superblock.block_size_exp + 10;

        // Check the magic value.
        if superblock.magic != MAGIC {
            logkf!(LogLevel::Error, "Bad or missing superblock");
            return Err(Errno::EIO);
        }

        // Set feature levels if rev >= 1.
        let mut feature_compat = 0u32;
        let mut feature_incompat = 0u32;
        let mut feature_ro_compat = 0u32;
        if superblock.rev_level >= 1 {
            feature_compat = superblock.feature_compat;
            feature_incompat = superblock.feature_incompat;
            feature_ro_compat = superblock.feature_ro_compat;

            // Check compatibility flags.
            if superblock.feature_incompat & !INCOMPAT_SUPPORTED != 0 {
                logkf!(
                    LogLevel::Error,
                    "Some INCOMPAT (0x{:08x}) features not supported (0x{:08x})",
                    superblock.feature_incompat as u32,
                    INCOMPAT_SUPPORTED
                );
                return Err(Errno::ENOTSUP);
            }
            if superblock.feature_ro_compat & !RO_COMPAT_SUPPORTED != 0 {
                logkf!(
                    LogLevel::Warning,
                    "Some RO_COMPAT (0x{:08x}) features not supported (0x{:08x})",
                    superblock.feature_ro_compat as u32,
                    RO_COMPAT_SUPPORTED
                );
            }
            if superblock.feature_compat & !COMPAT_SUPPORTED != 0 {
                logkf!(
                    LogLevel::Info,
                    "Some COMPAT (0x{:08x}) features not supported (0x{:08x})",
                    superblock.feature_compat as u32,
                    COMPAT_SUPPORTED
                );
            }
        }

        if superblock.frag_size_exp != superblock.block_size_exp {
            logkf!(
                LogLevel::Error,
                "Different block size and fragment size is not supported"
            );
            return Err(Errno::ENOTSUP);
        }

        // Construct VFS.
        let block_groups = superblock.block_count.div_ceil(superblock.blocks_per_group);
        let vfs = Box::try_new(E2Fs {
            first_data_block: superblock.first_data_block,
            block_size_exp,
            bgdt_offset: (superblock.first_data_block as u64 + 1) << block_size_exp,
            feature_compat,
            feature_incompat,
            feature_ro_compat,
            block_groups,
            media,
            block_group_desc: Mutex::new(BTreeMap::new()),
            dirty_bgdt_ents: Mutex::new(BTreeSet::new()),
            inodes_per_group: superblock.inodes_per_group,
            inode_size: superblock.inode_size,
            blocks_per_group: superblock.blocks_per_group,
            free_blocks: AtomicU32::new(0),
            free_inodes: AtomicU32::new(0),
        })?;

        // Test the number of free blocks and inodes.
        for group in 0..block_groups {
            let group = vfs.get_block_group(group)?;
            let guard = group.desc.unintr_lock();
            vfs.free_blocks
                .fetch_add(guard.free_block_count as u32, Ordering::Relaxed);
            vfs.free_inodes
                .fetch_add(guard.free_inode_count as u32, Ordering::Relaxed);
        }

        Ok(Box::<dyn VfsOps>::from(vfs))
    }
}

fn register_e2fs() {
    FSDRIVERS
        .unintr_lock()
        .insert("ext2".into(), Box::new(E2FsDriver {}));
}

register_kmodule!(ext2, [1, 0, 0], register_e2fs);
