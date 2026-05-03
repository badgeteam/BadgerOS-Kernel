// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    any::Any,
    cell::UnsafeCell,
    hint::unlikely,
    ops::Range,
    ptr,
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
};

use alloc::{
    boxed::Box,
    collections::btree_map::BTreeMap,
    sync::{Arc, Weak},
    vec::Vec,
};
use mflags::MFlags;

use super::{
    Dirent, File, MakeFileSpec, NodeType, SeekMode, Stat, UnlinkMode, media::Media, oflags,
    sysimpl::DentBuffer,
};
use crate::{
    LogLevel,
    bindings::{
        device::BaseDevice,
        error::{EResult, Errno},
    },
    filesystem::fifo::FifoShared,
    kernel::sync::mutex::{Mutex, MutexGuard, SharedMutexGuard},
    process::usercopy::{UserSlice, UserSliceMut},
};

/// Offset and flags for [`VfsFile`].
pub struct FlagsAndOffset {
    pub offset: u64,
    pub flags: u32,
}

/// A file that is stored in a [`Vfs`].
pub struct VfsFile {
    /// Underlying vnode.
    pub(super) vnode: Arc<VNode>,
    /// Current file position.
    pub(super) flags: Mutex<FlagsAndOffset>,
}

impl VfsFile {
    /// Implementation of append-mode writes.
    fn append_write(
        &self,
        mut flags: MutexGuard<'_, FlagsAndOffset>,
        wdata: UserSlice<'_, u8>,
    ) -> EResult<usize> {
        let mut guard = self.vnode.mtx.lock()?;
        let ops = &mut guard.ops;
        let old_size = ops.get_size(&self.vnode);
        let new_size = old_size
            .checked_add(wdata.len() as u64)
            .ok_or(Errno::ENOSPC)?;
        ops.resize(&self.vnode, new_size)?;
        flags.offset = new_size;
        ops.write(&self.vnode, old_size, wdata).map(|_| wdata.len())
    }

    /// Implementation of non-append writes.
    fn regular_write(
        &self,
        mut flags: MutexGuard<'_, FlagsAndOffset>,
        wdata: UserSlice<'_, u8>,
    ) -> EResult<usize> {
        let mut guard = self.vnode.mtx.lock_shared()?;
        let size = guard.ops.get_size(&self.vnode);

        let new_off = flags
            .offset
            .checked_add(wdata.len() as u64)
            .ok_or(Errno::ENOSPC)?;

        if new_off > size {
            // The file must be resized.
            drop(guard);
            let mut mut_guard = self.vnode.mtx.lock()?;
            mut_guard.ops.resize(&self.vnode, new_off)?;
            drop(mut_guard);
            guard = self.vnode.mtx.lock_shared()?;
        }
        // Offset updated successfully; perform write.
        flags.offset = new_off;
        return guard
            .ops
            .write(&self.vnode, flags.offset, wdata)
            .map(|_| wdata.len());
    }
}

impl File for VfsFile {
    fn get_flags(&self) -> u32 {
        self.flags.unintr_lock_shared().flags
    }

    fn set_flags(&self, newfl: u32) -> EResult<()> {
        self.flags.unintr_lock().flags = newfl;
        Ok(())
    }

    fn get_dirents(&self, buffer: &mut DentBuffer<'_>) -> EResult<()> {
        let mut flags = self.flags.lock()?;
        if flags.flags & oflags::READ_ONLY == 0 {
            return Err(Errno::EBADF);
        } else if self.vnode.type_ != NodeType::Directory {
            return Err(Errno::ENOTDIR);
        }
        let guard = self.vnode.mtx.lock_shared()?;
        flags.offset = guard.ops.get_dirents(&self.vnode, flags.offset, buffer)?;
        Ok(())
    }

    fn get_device(&self) -> Option<BaseDevice> {
        self.vnode
            .mtx
            .unintr_lock_shared()
            .ops
            .get_device(&self.vnode)
    }

    fn get_part_offset(&self) -> Option<Range<u64>> {
        self.vnode
            .mtx
            .unintr_lock_shared()
            .ops
            .get_part_offset(&self.vnode)
    }

    fn stat(&self) -> EResult<Stat> {
        Ok(Stat {
            ino: self.vnode.ino,
            ..self.vnode.mtx.lock_shared()?.ops.stat(&self.vnode)?
        })
    }

    fn tell(&self) -> EResult<u64> {
        Ok(self.flags.unintr_lock_shared().offset)
    }

    fn seek(&self, mode: SeekMode, offset: i64) -> EResult<u64> {
        let guard = self.vnode.mtx.lock_shared()?;
        let mut flags = self.flags.lock()?;
        let size = guard.ops.get_size(&self.vnode);

        let new_off = match mode {
            SeekMode::Set => offset.clamp(0, size as i64),
            SeekMode::Cur => offset
                .saturating_add(flags.offset as i64)
                .clamp(0, size as i64),
            SeekMode::End => offset.saturating_add(size as i64).clamp(0, size as i64),
        } as u64;
        flags.offset = new_off;

        Ok(new_off)
    }

    fn write(&self, wdata: UserSlice<'_, u8>) -> EResult<usize> {
        let flags = self.flags.lock()?;
        if flags.flags & oflags::WRITE_ONLY == 0 {
            Err(Errno::EBADF)
        } else if self.vnode.type_ == NodeType::Directory {
            return Err(Errno::EISDIR);
        } else if self.vnode.vfs.is_read_only() {
            Err(Errno::EROFS)
        } else if flags.flags & oflags::APPEND != 0 {
            self.vnode.vfs.check_eio(self.append_write(flags, wdata))
        } else {
            self.vnode.vfs.check_eio(self.regular_write(flags, wdata))
        }
    }

    fn read(&self, mut rdata: UserSliceMut<'_, u8>) -> EResult<usize> {
        let mut flags = self.flags.lock()?;
        if flags.flags & oflags::READ_ONLY == 0 {
            return Err(Errno::EBADF);
        } else if self.vnode.type_ == NodeType::Directory {
            return Err(Errno::EISDIR);
        }

        // Get file ops and size.
        let guard = self.vnode.mtx.lock_shared()?;
        let size = guard.ops.get_size(&self.vnode);

        // Increment offset and determine read count.
        let offset = flags.offset;
        let readlen = (rdata.len() as u64).min(size.saturating_sub(offset)) as usize;
        flags.offset = offset + readlen as u64;

        // Perform read on vnode ops.
        guard
            .ops
            .read(&self.vnode, offset, rdata.subslice_mut(0..readlen))?;
        Ok(readlen)
    }

    fn resize(&self, size: u64) -> EResult<()> {
        let mut flags = self.flags.lock()?;
        if flags.flags & oflags::WRITE_ONLY == 0 {
            return Err(Errno::EBADF);
        } else if self.vnode.vfs.is_read_only() {
            return Err(Errno::EROFS);
        }
        let mut guard = self.vnode.mtx.lock()?;
        self.vnode
            .vfs
            .check_eio(guard.ops.resize(&self.vnode, size))?;
        flags.offset = flags.offset.min(size);
        Ok(())
    }

    fn sync(&self) -> EResult<()> {
        self.vnode
            .vfs
            .check_eio(self.vnode.mtx.lock_shared()?.ops.sync(&self.vnode))
    }

    fn get_vnode(&self) -> Option<Arc<VNode>> {
        Some(self.vnode.clone())
    }
}

#[rustfmt::skip]
pub mod vnflags {
    /// VNode is removed from the filesystem.
    pub const REMOVED: u32 = 0x0000_0001;
}

/// [`VNode`] operations and flags.
pub struct VNodeMtxInner {
    /// VFS implementation of file operations.
    pub(super) ops: Box<dyn VNodeOps>,
    /// Dirent cache associated.
    pub(super) dentcache: Option<Arc<DentCache>>,
    /// VNode flags.
    pub(super) flags: u32,
}

/// A virtual generalization of inodes. Multiple [`super::File`]s may refer to one vnode.
/// TODO: A mechanism to mark a VNode as unlinked so it cannot be modified (flag exists but logic doesn't).
pub struct VNode {
    /// VNode operations and flags.
    pub(super) mtx: Mutex<VNodeMtxInner>,
    /// Inode number on the parent filesystem.
    pub(super) ino: u64,
    /// VFS on which this VNode exists.
    pub(super) vfs: Arc<Vfs>,
    /// VNode flags.
    pub(super) flags: AtomicU32,
    /// What kind of node this is.
    pub(super) type_: NodeType,
    /// Shared FIFO data.
    pub(super) fifo: Option<Arc<FifoShared>>,
    /// Deny writes counter set by the executable loader.
    pub denywrite: AtomicU32,
}

impl VNode {
    /// Lock the ops mutex shared and convert to type `U`.
    /// # Panics
    /// - If the ops are not of type `U`
    #[inline(always)]
    pub(super) fn get_ops_as<'a, U: VNodeOps>(self: &'a VNode) -> SharedMutexGuard<'a, U> {
        self.mtx
            .unintr_lock_shared()
            .convert(|x| (x.ops.as_ref() as &dyn Any).downcast_ref().unwrap())
    }

    /// Get the filesystem mounted here, if any.
    pub fn get_mounted(&self) -> Option<Arc<Vfs>> {
        self.mtx
            .unintr_lock_shared()
            .dentcache
            .clone()?
            .type_
            .as_dir()?
            .unintr_lock_shared()
            .mounted
            .clone()
    }

    /// Follow mounts here.
    pub fn follow_mounts(self: &Arc<Self>) -> Arc<Self> {
        let dentcache = self.mtx.unintr_lock_shared().dentcache.clone();
        let dentcache = if let Some(dentcache) = dentcache {
            dentcache
        } else {
            return self.clone();
        };
        dentcache
            .follow_mounts()
            .open_vnode()
            .unwrap_or(self.clone())
    }

    /// Get the VFS that this is the root directory of, if any.
    pub fn is_vfs_root(&self) -> Option<Arc<Vfs>> {
        self.mtx
            .unintr_lock_shared()
            .dentcache
            .as_ref()?
            .parent
            .is_none()
            .then(|| self.vfs.clone())
    }
}

impl Drop for VNode {
    fn drop(&mut self) {
        self.mtx.unintr_lock().ops.close(self);
        self.vfs.vnodes.unintr_lock().remove(&self.ino);
    }
}

/// Abstract vnode operations.
pub trait VNodeOps: Any {
    /// Get the associated character device, if any.
    fn get_device(&self, _arc_self: &Arc<VNode>) -> Option<BaseDevice> {
        None
    }
    /// Get the partition offset and size that this file represents, if any.
    fn get_part_offset(&self, _arc_self: &Arc<VNode>) -> Option<Range<u64>> {
        None
    }

    /// Read directory entries into the buffer.
    fn get_dirents(
        &self,
        arc_self: &Arc<VNode>,
        offset: u64,
        buffer: &mut DentBuffer<'_>,
    ) -> EResult<u64>;
    /// Write data to the file.
    fn write(&self, arc_self: &Arc<VNode>, offset: u64, wdata: UserSlice<'_, u8>) -> EResult<()>;
    /// Read data from the file.
    fn read(&self, arc_self: &Arc<VNode>, offset: u64, rdata: UserSliceMut<'_, u8>) -> EResult<()>;
    /// Resize the file.
    fn resize(&mut self, arc_self: &Arc<VNode>, new_size: u64) -> EResult<()>;

    /// Find a directory entry.
    fn find_dirent(&self, arc_self: &Arc<VNode>, name: &[u8]) -> EResult<Dirent>;
    /// Get all directory entries.
    // fn get_dirents(&self, arc_self: &Arc<VNode>) -> EResult<Vec<Dirent>>;
    /// Unlink a node from this directory.
    /// Uses POSIX `rmdir` semantics iff `is_rmdir`, otherwise POSIX unlink semantics.
    fn unlink(
        &mut self,
        arc_self: &Arc<VNode>,
        name: &[u8],
        mode: UnlinkMode,
        unlinked_vnode: Option<Arc<VNode>>,
    ) -> EResult<()>;
    /// Link an existing inode to this directory.
    fn link(&mut self, arc_self: &Arc<VNode>, name: &[u8], inode: &VNode) -> EResult<()>;
    /// Create a new file in this directory.
    fn make_file(
        &mut self,
        arc_self: &Arc<VNode>,
        name: &[u8],
        spec: MakeFileSpec,
    ) -> EResult<(Dirent, Box<dyn VNodeOps>)>;
    /// Rename a file within this directory.
    /// See [`VfsOps::rename`] for renaming between two different directories.
    fn rename(
        &mut self,
        arc_self: &Arc<VNode>,
        old_name: &[u8],
        new_name: &[u8],
    ) -> EResult<Dirent>;

    /// Read the link if this is a symlink.
    fn readlink(&self, arc_self: &Arc<VNode>) -> EResult<Box<[u8]>>;
    /// Get this node's stat buffer.
    /// This function need not set [`Stat::ino`]; it is copied from the [`VNode`].
    fn stat(&self, arc_self: &Arc<VNode>) -> EResult<Stat>;
    /// Get this node's inode number.
    /// Called only once during construction of the VNode and therefor doesn't have `arc_self`.
    fn get_inode(&self) -> u64;
    /// Get the current size of the file.
    fn get_size(&self, arc_self: &Arc<VNode>) -> u64;
    /// Get the type of nod this is.
    fn get_type(&self, arc_self: &Arc<VNode>) -> NodeType;
    /// Sync the underlying caches to disk.
    fn sync(&self, arc_self: &Arc<VNode>) -> EResult<()>;

    /// Called in the [`Drop`] implementation of [`VNode`].
    fn close(&mut self, _vnode_self: &VNode) {}
}

impl dyn VNodeOps + '_ {
    /// Write data to the file.
    pub fn writek(&self, arc_self: &Arc<VNode>, offset: u64, wdata: &[u8]) -> EResult<()> {
        self.write(arc_self, offset, UserSlice::new_kernel(wdata))
    }

    /// Read data from the file.
    pub fn readk(&self, arc_self: &Arc<VNode>, offset: u64, rdata: &mut [u8]) -> EResult<()> {
        self.read(arc_self, offset, UserSliceMut::new_kernel_mut(rdata))
    }
}

#[rustfmt::skip]
pub mod mflags {
    /// Mounted filesystem flags.
    pub type MFlags = u32;
    /// Filesystem is read-only.
    pub const READ_ONLY: u32 = 0x0000_0001;
    /// Do not follow symbolic links.
    pub const NOFOLLOW:  u32 = 0x0000_0020;
    /// Try to cancel pending I/O operations; only supported on certain filesystems.
    pub const FORCE:     u32 = 0x0001_0000;
    /// Lazily unmount; remove the filesystem from the tree now, and wait with unmount until open handles are closed.
    pub const DETACH:    u32 = 0x0002_0000;
}

/// A mounted virtual filesystem.
pub struct Vfs {
    /// Instance of the filesystem driver.
    pub(super) ops: Mutex<Box<dyn VfsOps>>,
    /// Dictionary of VNodes that are opened in this VFS.
    pub(super) vnodes: Mutex<BTreeMap<u64, Weak<VNode>>>,
    /// Handle of the root directory.
    pub(super) root: UnsafeCell<Option<Arc<VNode>>>,
    /// Mountpoint of this VFS.
    pub(super) mountpoint: Option<Arc<VNode>>,
    /// Mounted filesystem flags.
    pub(super) flags: AtomicU32,
    /// Fake inode counter for filesystems that do not implement inodes.
    pub(super) next_fake_ino: AtomicU64,
}
unsafe impl Sync for Vfs {}

impl Vfs {
    pub fn is_read_only(&self) -> bool {
        self.flags.load(Ordering::Relaxed) & mflags::READ_ONLY != 0
    }

    /// Lock the ops mutex shared and convert to type `U`.
    /// # Panics
    /// - If the ops are not of type `U`
    #[inline(always)]
    pub(super) fn get_ops_as<'a, U: VfsOps>(self: &'a Vfs) -> SharedMutexGuard<'a, U> {
        self.ops
            .unintr_lock_shared()
            .convert(|x| (x.as_ref() as &dyn Any).downcast_ref().unwrap())
    }

    /// Helper function to get [`Vfs::root`], which must already be initialized.
    pub(super) fn root(&self) -> Arc<VNode> {
        unsafe {
            self.root
                .as_ref_unchecked()
                .clone()
                .unwrap_unchecked()
                .clone()
        }
    }

    /// Try to get an existing vnode.
    pub(super) fn get_vnode(&self, inode: u64) -> Option<Arc<VNode>> {
        self.vnodes.unintr_lock_shared().get(&inode)?.upgrade()
    }

    /// Try to open a vnode.
    /// The caller must guarantee `dirent` is up-to-date.
    fn open(
        self: &Arc<Self>,
        dirent: &Dirent,
        dentcache: Option<Arc<DentCache>>,
    ) -> EResult<Arc<VNode>> {
        let uses_inodes = self.ops.lock_shared()?.uses_inodes();
        if let Some(vnode) = self.get_vnode(dirent.ino) {
            return Ok(vnode);
        }

        let mut guard = self.vnodes.lock()?;
        if uses_inodes {
            if let Some(vnode) = try { guard.get(&dirent.ino)?.upgrade()? } {
                // Race condition: Another thread opened the vnode in the mean time.
                return Ok(vnode);
            }
        } // Omitting this for inode-less filesystems works because the `DentCache` also locks its `vnode` field.

        // Call the filesystem to open the vnode.
        let ops = self.ops.lock_shared()?.open(self, dirent)?;

        let fifo = (dirent.type_ == NodeType::Fifo).then(|| FifoShared::new());
        let ino = if uses_inodes {
            dirent.ino
        } else {
            self.next_fake_ino.fetch_add(1, Ordering::Relaxed)
        };
        let vnode = Arc::try_new(VNode {
            mtx: Mutex::new(VNodeMtxInner {
                ops,
                flags: 0,
                dentcache,
            }),
            ino,
            vfs: self.clone(),
            flags: AtomicU32::new(0),
            type_: dirent.type_,
            fifo,
            denywrite: AtomicU32::new(0),
        })?;

        // Insert the new vnode.
        // This is done for inode-less filesystems so that we can tell whether any files are open.
        guard.insert(ino, Arc::downgrade(&vnode));

        Ok(vnode)
    }

    #[inline(never)]
    /// Called if an I/O error happens on this VFS.
    pub(super) fn check_eio_failed(&self) {
        if self.flags.fetch_or(mflags::READ_ONLY, Ordering::Relaxed) & mflags::READ_ONLY == 0 {
            logkf!(
                LogLevel::Error,
                "I/O error on filesystem; marking read-only"
            );
        }
    }

    #[inline(always)]
    /// Mark the VFS as readonly and raise a warning if `result` is [`Errno::EIO`].
    pub(super) fn check_eio<T>(&self, result: EResult<T>) -> EResult<T> {
        if unlikely(match &result {
            Err(x) => *x == Errno::EIO,
            _ => false,
        }) {
            self.check_eio_failed();
        }
        result
    }
}

impl Drop for Vfs {
    fn drop(&mut self) {
        self.ops.unintr_lock().umount(self);
    }
}

/// Filesystem-wide operations for a [`Vfs`]; instance of a [`VfsDriver`].
pub trait VfsOps: Sync + Any {
    /// Get the media that this VFS uses.
    fn media(&self) -> Option<&Media>;
    /// Whether this type of filesystem has inode numers.
    /// If disabled, inode numbers will be spoofed when a [`VNode`] is opened.
    fn uses_inodes(&self) -> bool;
    /// Whether this filesystem must be mounted as read-only.
    fn read_only(&self) -> bool {
        false
    }

    /// Open the root directory.
    fn open_root(&self, self_arc: &Arc<Vfs>) -> EResult<Box<dyn VNodeOps>>;
    /// Open a file or directory.
    /// The caller must guarantee `dirent` is up-to-date.
    fn open(&self, self_arc: &Arc<Vfs>, dirent: &Dirent) -> EResult<Box<dyn VNodeOps>>;
    /// Rename between two different directories.
    /// See [`VNodeOps::rename`] for renaming within a single directory.
    fn rename(
        &self,
        self_arc: &Arc<Vfs>,
        src_dir: &Arc<VNode>,
        src_name: &[u8],
        src_mutexinner: &mut VNodeMtxInner,
        dest_dir: &Arc<VNode>,
        dest_name: &[u8],
        dest_mutexinner: &mut VNodeMtxInner,
    ) -> EResult<Dirent>;

    /// Sync the cached changes that are not file- or inode-specific to disk.
    fn sync(&self) -> EResult<()> {
        Ok(())
    }
    /// Called in the [`Drop`] implementation of [`Vfs`].
    fn umount(&mut self, _vfs_self: &Vfs) {
        if let Err(x) = self.sync() {
            logkf!(LogLevel::Error, "Failed to sync: {}", x);
        }
    }
}

/// A filesystem driver.
pub trait VfsDriver {
    /// Detect the filesystem on some medium.
    fn detect(&self, media: &Media) -> EResult<bool>;
    /// Mount the filesystem on some medium.
    /// Expected to log errors if they are caused by invalid parameters.
    fn mount(&self, media: Option<Media>, mflags: MFlags) -> EResult<Box<dyn VfsOps>>;
}

/// Data associated with dirent caches for directories.
#[derive(Clone)]
pub(super) struct DentCacheDir {
    /// Child dentcache nodes.
    pub children: BTreeMap<Box<[u8]>, Weak<DentCache>>,
    /// Filesystem mounted at this location.
    pub mounted: Option<Arc<Vfs>>,
}

impl DentCacheDir {
    pub const EMPTY: DentCacheDir = DentCacheDir {
        children: BTreeMap::new(),
        mounted: None,
    };

    /// Check just the directory cache for this name, which must be the same entry as specified.
    /// This is used to check for race condition; raises [`Errno::ETIMEDOUT`] if the entry is not present.
    pub fn check_for_entry(&self, what: &DentCache) -> EResult<()> {
        if !ptr::addr_eq(
            self.children
                .get(&what.dirent.name)
                .ok_or(Errno::ETIMEDOUT)? // This would be a race condition; ETIMEDOUT here causes a retry.
                .upgrade()
                .ok_or(Errno::ETIMEDOUT)? // Same.
                .as_ref(),
            what,
        ) {
            // Also a race condition.
            return Err(Errno::ETIMEDOUT);
        }
        Ok(())
    }
}

/// Possible types of dirent cache entry.
pub(super) enum DentCacheType {
    /// Explicitly does not exist.
    Negative,
    /// A directory.
    Directory(Mutex<DentCacheDir>),
    /// A symbolic link.
    Symlink(Box<[u8]>),
    /// Some other kind of file.
    File,
}

impl DentCacheType {
    pub fn as_dir(&self) -> Option<&Mutex<DentCacheDir>> {
        match self {
            Self::Directory(x) => Some(x),
            _ => None,
        }
    }

    pub const fn is_negative(&self) -> bool {
        match self {
            Self::Negative => true,
            _ => false,
        }
    }
}

/// A directory cache entry.
pub(super) struct DentCache {
    /// What kind of entry this is.
    pub type_: DentCacheType,
    /// The VFS this resides in.
    pub vfs: Arc<Vfs>,
    /// The parent dirent.
    pub parent: Option<Arc<DentCache>>,
    /// The vnode this is linked to, if any.
    pub vnode: Mutex<Option<Weak<VNode>>>,
    /// This cached dirent.
    pub dirent: Dirent,
}

impl DentCache {
    /// Get the real path this cache entry represents.
    pub fn realpath(self: &Arc<Self>) -> EResult<Vec<u8>> {
        let mut this = self.clone();
        let mut components = Vec::new();

        while let Some(parent) = this.parent.clone() {
            components.try_reserve(1)?;
            components.push(this.clone());
            this = parent;
        }

        let mut path = Vec::new();
        if components.len() == 0 {
            path.try_reserve(1)?;
            path.push(b'/');
        } else {
            for component in components.iter().rev() {
                path.try_reserve(component.dirent.name.len() + 1)?;
                path.push(b'/');
                path.extend(component.dirent.name.iter());
            }
        }

        Ok(path)
    }

    /// Read the symlink target.
    pub fn readlink(&self) -> EResult<&[u8]> {
        if let DentCacheType::Symlink(link) = &self.type_ {
            Ok(link)
        } else {
            Err(Errno::EINVAL)
        }
    }

    /// Look up a name in this directory.
    pub fn lookup(self: &Arc<Self>, component: &[u8]) -> EResult<Arc<DentCache>> {
        Self::lookup_impl(self.clone(), component)
    }

    #[inline(always)]
    /// Look up a name in this directory.
    fn lookup_impl(mut this: Arc<Self>, component: &[u8]) -> EResult<Arc<DentCache>> {
        // Assert this to be a directory.
        let mut cache = if let DentCacheType::Directory(x) = &this.type_ {
            x
        } else {
            return Err(Errno::ENOTDIR);
        };
        let mut guard = cache.lock_shared()?;

        // Handle mounted filesystems.
        while let Some(mounted) = guard.mounted.clone() {
            drop(guard);
            this = mounted.root().mtx.lock_shared()?.dentcache.clone().unwrap();
            cache = if let DentCacheType::Directory(x) = &this.type_ {
                x
            } else {
                unreachable!();
            };
            guard = cache.lock_shared()?;
        }

        // Handle `.` and `..` components.
        if component == b"." {
            return Ok(this.clone());
        } else if component == b".." {
            // Get the parent dir.
            if let Some(x) = &this.parent {
                return Ok(x.clone());
            }
            // Traverse back up to the parent VFS' mountpoint.
            drop(guard);
            while let Some(x) = this.vfs.mountpoint.clone() {
                this = x.mtx.lock_shared()?.dentcache.clone().unwrap();
            }
            // Try again to get the parent dir on the new VFS.
            if let Some(x) = &this.parent {
                return Ok(x.clone());
            } else {
                return Ok(this.clone());
            }
        }

        // Try to look up from the cache.
        if let Some(child) = guard.children.get(component) {
            if let Some(arc) = child.upgrade() {
                // It was cached already.
                return Ok(arc);
            }
        }
        drop(guard);

        // Read from the filesystem.
        let mut guard = cache.lock()?;
        if let Some(child) = guard.children.get(component) {
            if let Some(arc) = child.upgrade() {
                // Race condition: It was cached by another thread while the mutex was not held.
                return Ok(arc);
            }
        }
        let self_vnode = this.open_vnode()?;
        let dirent = match self_vnode
            .mtx
            .lock_shared()?
            .ops
            .find_dirent(&self_vnode, component)
        {
            Ok(x) => x,
            Err(x) => {
                if x == Errno::ENOENT {
                    // Directory exists but the file requested doesn't.
                    let value = Arc::try_new(DentCache {
                        type_: DentCacheType::Negative,
                        vfs: this.vfs.clone(),
                        parent: Some(this.clone()),
                        vnode: Mutex::new(None),
                        dirent: Dirent {
                            name: component.into(),
                            ..Default::default()
                        },
                    })?;
                    guard
                        .children
                        .insert(component.into(), Arc::downgrade(&value));

                    return Ok(value);
                } else {
                    return Err(x);
                }
            }
        };

        // Insert new entry.
        match dirent.type_ {
            NodeType::Directory => {
                let value = Arc::try_new(DentCache {
                    type_: DentCacheType::Directory(Mutex::new(DentCacheDir::EMPTY)),
                    parent: Some(this.clone()),
                    vnode: Mutex::new(None),
                    vfs: this.vfs.clone(),
                    dirent,
                })?;
                guard
                    .children
                    .insert(component.into(), Arc::downgrade(&value));

                Ok(value)
            }
            NodeType::Symlink => {
                // Read the symlink first.
                let vnode = self_vnode.vfs.open(&dirent, None)?;
                let name = vnode.mtx.lock_shared()?.ops.readlink(&self_vnode)?;

                let value = Arc::try_new(DentCache {
                    type_: DentCacheType::Symlink(name),
                    parent: Some(this.clone()),
                    vnode: Mutex::new(Some(Arc::downgrade(&vnode))),
                    vfs: this.vfs.clone(),
                    dirent,
                })?;
                guard
                    .children
                    .insert(component.into(), Arc::downgrade(&value));

                Ok(value)
            }
            _ => {
                // Neither directory nor symlink.
                let value = Arc::try_new(DentCache {
                    type_: DentCacheType::File,
                    parent: Some(this.clone()),
                    vnode: Mutex::new(None),
                    vfs: this.vfs.clone(),
                    dirent,
                })?;
                guard
                    .children
                    .insert(component.into(), Arc::downgrade(&value));

                Ok(value)
            }
        }
    }

    /// Follow any possible mounts on this dentcache.
    pub fn follow_mounts(self: &Arc<Self>) -> Arc<Self> {
        let mut this = self.clone();
        if let DentCacheType::Directory(cache) = &this.type_ {
            let mut cache = cache;
            let mut guard = cache.unintr_lock_shared();
            while let Some(mounted) = guard.mounted.clone() {
                drop(guard);
                this = mounted
                    .root()
                    .mtx
                    .unintr_lock_shared()
                    .dentcache
                    .clone()
                    .unwrap();
                cache = if let DentCacheType::Directory(x) = &this.type_ {
                    x
                } else {
                    unreachable!();
                };
                guard = cache.unintr_lock_shared();
            }
        }
        this
    }

    /// Un-follow mounts; returns the parent VFS dentcache if this is a VFS root.
    /// Returns self if this is the root directory.
    pub fn unfollow_mounts(self: &Arc<Self>) -> Arc<Self> {
        let mut this = self.clone();
        while self.parent.is_none()
            && let Some(mountpoint) = self.vfs.mountpoint.clone()
        {
            this = mountpoint
                .mtx
                .unintr_lock_shared()
                .dentcache
                .clone()
                .unwrap();
        }
        this
    }

    /// Whether this is the root of a VFS.
    pub fn is_vfs_root(&self) -> bool {
        self.parent.is_none()
    }

    /// Get or open the associated VNode.
    pub fn open_vnode(self: &Arc<Self>) -> EResult<Arc<VNode>> {
        let uses_inodes = self.vfs.ops.lock_shared()?.uses_inodes();
        if let Some(weak) = &*self.vnode.lock_shared()? {
            if let Some(arc) = weak.upgrade() {
                return Ok(arc);
            }
        }

        let mut guard = self.vnode.lock()?;
        if let Some(weak) = &*guard {
            if let Some(arc) = weak.upgrade() {
                return Ok(arc);
            }
        }

        let vnode = self.vfs.open(
            &self.dirent,
            if self.dirent.type_ == NodeType::Directory || !uses_inodes {
                // Must also provide for inode-less regular files so re-opening the same file will get the same VNode.
                Some(self.clone())
            } else {
                None
            },
        )?;

        *guard = Some(Arc::downgrade(&vnode));

        Ok(vnode)
    }
}
