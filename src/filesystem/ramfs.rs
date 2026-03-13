// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    any::Any,
    cell::UnsafeCell,
    ops::Range,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc, vec::Vec};

use crate::{
    LogLevel,
    badgelib::time::{AtomicTimespec, Timespec},
    bindings::{
        device::{
            BaseDevice, HasBaseDevice,
            class::{block::BlockDevice, char::CharDevice},
        },
        error::{EResult, Errno},
        spinlock::Spinlock,
    },
    cpu,
    filesystem::{VNodeMtxInner, vfs::mflags},
    kernel::sync::mutex::Mutex,
    process::usercopy::{UserSlice, UserSliceMut},
};

use super::{
    Dirent, FSDRIVERS, MakeFileSpec, NodeMode, NodeType, Stat,
    media::Media,
    vfs::{VNode, VNodeOps, Vfs, VfsDriver, VfsOps, mflags::MFlags},
};

/// A filesystem that is entirely resident in RAM.
pub struct RamFs {
    /// Inodes table.
    inodes: Mutex<BTreeMap<u64, Arc<UnsafeCell<RamINode>>>>,
    /// Inode number counter.
    ino_ctr: AtomicU64,
    /// Allows for device files.
    allow_devfiles: bool,
}
unsafe impl Sync for RamFs {}

impl RamFs {
    pub fn new(allow_devfiles: bool) -> EResult<Self> {
        let fs = RamFs {
            allow_devfiles,
            inodes: Mutex::new(BTreeMap::new()),
            ino_ctr: AtomicU64::new(2),
        };
        let now = Timespec::now();
        let root = Arc::try_new(UnsafeCell::new(RamINode {
            data: RamFsData::Directory(BTreeMap::new()),
            size: AtomicUsize::new(0),
            links: Spinlock::new(0),
            ino: 1,
            atim: AtomicTimespec::new(now),
            mtim: AtomicTimespec::new(now),
            ctim: AtomicTimespec::new(now),
        }))?;
        fs.inodes.unintr_lock().insert(1, root);
        Ok(fs)
    }

    fn open_impl(&self, ino: u64) -> EResult<Box<dyn VNodeOps>> {
        let inode = self
            .inodes
            .lock_shared()?
            .get(&ino)
            .ok_or(Errno::EIO)?
            .clone();
        Ok(Box::<dyn VNodeOps>::from(Box::try_new(RamVNode { inode })?))
    }
}

impl VfsOps for RamFs {
    fn media(&self) -> Option<&Media> {
        None
    }

    fn uses_inodes(&self) -> bool {
        true
    }

    fn open_root(&self, _self_arc: &Arc<Vfs>) -> EResult<Box<dyn VNodeOps>> {
        self.open_impl(1)
    }

    fn open(&self, _self_arc: &Arc<Vfs>, dirent: &Dirent) -> EResult<Box<dyn VNodeOps>> {
        self.open_impl(dirent.ino)
    }

    fn rename(
        &self,
        _self_arc: &Arc<Vfs>,
        src_dir: &Arc<VNode>,
        src_name: &[u8],
        _src_mutexinner: &mut VNodeMtxInner,
        dest_dir: &Arc<VNode>,
        dest_name: &[u8],
        _dest_mutexinner: &mut VNodeMtxInner,
    ) -> EResult<Dirent> {
        // Downcast trait objects into RamVNode.
        let mut src_ops = src_dir.mtx.lock()?;
        let mut dest_ops = dest_dir.mtx.lock()?;
        let src_ramnode = (&mut *src_ops as &mut dyn Any)
            .downcast_mut::<RamVNode>()
            .unwrap();
        let dest_ramnode = (&mut *dest_ops as &mut dyn Any)
            .downcast_mut::<RamVNode>()
            .unwrap();

        // Get RamINode refs.
        let src_inode = unsafe { src_ramnode.inode.as_mut_unchecked() };
        let dest_inode = unsafe { dest_ramnode.inode.as_mut_unchecked() };

        // Get the source dirent.
        let mut entry = src_inode
            .data
            .as_directory_mut()
            .unwrap()
            .remove(src_name)
            .ok_or(Errno::ENOENT)?;

        // Check if destination already exists.
        let dest_dir_map = dest_inode.data.as_directory_mut().unwrap();
        if dest_dir_map.contains_key(dest_name) {
            return Err(Errno::EEXIST);
        }

        // Insert the entry under the new name.
        entry.name = dest_name.into(); // TODO: OOM handling.
        dest_dir_map.insert(dest_name.into(), entry.clone());

        Ok(entry)
    }
}

/// Data stored in a [`RamINode`].
enum RamFsData {
    /// Named pipe.
    Fifo,
    /// Character device.
    CharDev(CharDevice),
    /// Directory.
    Directory(BTreeMap<Box<[u8]>, Dirent>),
    /// Block device.
    BlockDev((BlockDevice, Option<Range<u64>>)),
    /// Regular file.
    Regular(Vec<u8>),
    /// Symbolic link.
    Symlink(Box<[u8]>),
    /// UNIX domain socket.
    UnixSocket,
}

impl RamFsData {
    /// Get as directory.
    fn as_directory(&self) -> Option<&BTreeMap<Box<[u8]>, Dirent>> {
        match self {
            Self::Directory(x) => Some(x),
            _ => None,
        }
    }
    /// Get as directory (mutable).
    fn as_directory_mut(&mut self) -> Option<&mut BTreeMap<Box<[u8]>, Dirent>> {
        match self {
            Self::Directory(x) => Some(x),
            _ => None,
        }
    }
    /// Get as regular file.
    fn as_regular(&self) -> Option<&Vec<u8>> {
        match self {
            Self::Regular(x) => Some(x),
            _ => None,
        }
    }
    /// Get as regular file (mutable).
    fn as_regular_mut(&mut self) -> Option<&mut Vec<u8>> {
        match self {
            Self::Regular(x) => Some(x),
            _ => None,
        }
    }
    /// Get as symlink.
    fn as_symlink(&self) -> Option<&Box<[u8]>> {
        match self {
            Self::Symlink(x) => Some(x),
            _ => None,
        }
    }

    /// Get the matching [`NodeType`].
    fn node_type(&self) -> NodeType {
        match self {
            RamFsData::Fifo => NodeType::Fifo,
            RamFsData::CharDev(_) => NodeType::CharDev,
            RamFsData::Directory(_) => NodeType::Directory,
            RamFsData::BlockDev(_) => NodeType::BlockDev,
            RamFsData::Regular(_) => NodeType::Regular,
            RamFsData::Symlink(_) => NodeType::Symlink,
            RamFsData::UnixSocket => NodeType::UnixSocket,
        }
    }
}

/// A [`RamFS`] inode.
struct RamINode {
    /// The data stored in this inode.
    data: RamFsData,
    /// Number of bytes in use excluding data structure overhead.
    size: AtomicUsize,
    /// Number of hard links to this inode.
    links: Spinlock<u16>,
    /// Inode number.
    ino: u64,
    /// Time of last access. On BadgerOS, only updated when modified or created.
    atim: AtomicTimespec,
    /// Time of last modification.
    mtim: AtomicTimespec,
    /// Time of last status change.
    ctim: AtomicTimespec,
}

/// VNode wrapper for a [`RamINode`].
struct RamVNode {
    inode: Arc<UnsafeCell<RamINode>>,
}

impl VNodeOps for RamVNode {
    fn get_device(&self, _arc_self: &Arc<VNode>) -> Option<BaseDevice> {
        let inode = unsafe { self.inode.as_ref_unchecked() };
        match &inode.data {
            RamFsData::CharDev(dev) => Some(dev.as_base().clone()),
            RamFsData::BlockDev(dev) => Some(dev.0.as_base().clone()),
            _ => None,
        }
    }

    fn get_part_offset(&self, _arc_self: &Arc<VNode>) -> Option<Range<u64>> {
        let inode = unsafe { self.inode.as_ref_unchecked() };
        if let RamFsData::BlockDev(dev) = &inode.data {
            dev.1.clone()
        } else {
            None
        }
    }

    fn write(&self, _arc_self: &Arc<VNode>, offset: u64, wdata: UserSlice<'_, u8>) -> EResult<()> {
        let offset: usize = offset.try_into().map_err(|_| Errno::EIO)?;
        let inode = unsafe { self.inode.as_mut_unchecked() };
        let regular = inode.data.as_regular_mut().ok_or(Errno::EINVAL)?;
        if offset.checked_add(wdata.len()).ok_or(Errno::EIO)? > regular.len() {
            return Err(Errno::EIO);
        }
        Ok(wdata.read_multiple(offset, &mut regular[offset..offset + wdata.len()])?)
    }

    fn read(
        &self,
        _arc_self: &Arc<VNode>,
        offset: u64,
        mut rdata: UserSliceMut<'_, u8>,
    ) -> EResult<()> {
        let offset: usize = offset.try_into().map_err(|_| Errno::EIO)?;
        let inode = unsafe { self.inode.as_ref_unchecked() };
        let regular = inode.data.as_regular().ok_or(Errno::EINVAL)?;
        if offset.checked_add(rdata.len()).ok_or(Errno::EIO)? > regular.len() {
            return Err(Errno::EIO);
        }
        Ok(rdata.write_multiple(0, &regular[offset..offset + rdata.len()])?)
    }

    fn resize(&mut self, _arc_self: &Arc<VNode>, new_size: u64) -> EResult<()> {
        let new_size: usize = new_size.try_into().map_err(|_| Errno::ENOSPC)?;
        let inode = unsafe { self.inode.as_mut_unchecked() };
        let regular = inode.data.as_regular_mut().ok_or(Errno::EINVAL)?;
        regular
            .try_reserve(new_size.saturating_sub(regular.len()))
            .map_err(|_| Errno::ENOSPC)?;
        regular.resize(new_size, 0);
        inode.size.store(new_size, Ordering::Relaxed);
        Ok(())
    }

    fn find_dirent(&self, _arc_self: &Arc<VNode>, name: &[u8]) -> EResult<Dirent> {
        let inode = unsafe { self.inode.as_ref_unchecked() };
        let directory = inode.data.as_directory().ok_or(Errno::EINVAL)?;
        directory.get(name).ok_or(Errno::ENOENT).cloned()
    }

    fn get_dirents(&self, _arc_self: &Arc<VNode>) -> EResult<Vec<Dirent>> {
        let inode = unsafe { self.inode.as_ref_unchecked() };
        let directory = inode.data.as_directory().ok_or(Errno::EINVAL)?;
        // TODO: OOM handling
        Ok(directory.values().map(Dirent::clone).collect())
    }

    fn unlink(
        &mut self,
        arc_self: &Arc<VNode>,
        name: &[u8],
        is_rmdir: bool,
        _unlinked_vnode: Option<Arc<VNode>>,
    ) -> EResult<()> {
        let ramfs = arc_self.vfs.get_ops_as::<RamFs>();
        let inode = unsafe { self.inode.as_mut_unchecked() };
        let directory = inode.data.as_directory_mut().ok_or(Errno::EINVAL)?;

        let dirent = directory.remove(name).ok_or(Errno::ENOENT)?;
        let ino = ramfs
            .inodes
            .unintr_lock_shared()
            .get(&dirent.ino)
            .unwrap()
            .clone();
        let ino = unsafe { ino.as_ref_unchecked() };

        if let RamFsData::Directory(data) = &ino.data {
            if !is_rmdir {
                return Err(Errno::EISDIR);
            } else if data.len() != 0 {
                return Err(Errno::ENOTEMPTY);
            }
        } else if is_rmdir {
            return Err(Errno::ENOTDIR);
        }

        // Pop refcount.
        assert!(unsafe { cpu::irq::disable() });
        let mut link_guard = ino.links.lock();
        let prev_links = *link_guard;
        *link_guard -= 1;
        drop(link_guard);
        unsafe { cpu::irq::enable() };

        if prev_links == 1 {
            // Last link removed.
            ramfs.inodes.unintr_lock().remove(&dirent.ino).unwrap();
        }

        Ok(())
    }

    fn link(&mut self, arc_self: &Arc<VNode>, name: &[u8], inode: &VNode) -> EResult<()> {
        let ramfs = arc_self.vfs.get_ops_as::<RamFs>();
        let dir_inode = unsafe { self.inode.as_mut_unchecked() };
        let directory = dir_inode.data.as_directory_mut().ok_or(Errno::EINVAL)?;

        if directory.contains_key(name) {
            return Err(Errno::EEXIST);
        }

        let ram_inode = ramfs
            .inodes
            .lock_shared()?
            .get(&inode.ino)
            .cloned()
            .ok_or(Errno::EIO)?;

        let ram_inode = unsafe { ram_inode.as_mut_unchecked() };
        assert!(unsafe { cpu::irq::disable() });
        let link_res = {
            let mut links = ram_inode.links.lock();
            assert!(*links > 0);
            if *links >= u16::MAX {
                Err(Errno::EMLINK)
            } else {
                *links += 1;
                Ok(())
            }
        };
        unsafe { cpu::irq::enable() };
        link_res?;

        directory.insert(
            name.into(),
            Dirent {
                ino: ram_inode.ino,
                type_: ram_inode.data.node_type(),
                name: name.into(),
                dirent_disk_off: 0,
                dirent_off: 0,
            },
        );

        Ok(())
    }

    fn make_file(
        &mut self,
        arc_self: &Arc<VNode>,
        name: &[u8],
        spec: MakeFileSpec,
    ) -> EResult<(Dirent, Box<dyn VNodeOps>)> {
        let ramfs = arc_self.vfs.get_ops_as::<RamFs>();

        let inode = unsafe { self.inode.as_mut_unchecked() };
        let directory = inode.data.as_directory_mut().ok_or(Errno::EINVAL)?;

        if let Some(_) = directory.get(name) {
            return Err(Errno::EEXIST);
        }

        if !ramfs.allow_devfiles {
            match &spec {
                MakeFileSpec::CharDev(_) => return Err(Errno::EINVAL),
                MakeFileSpec::BlockDev(_) => return Err(Errno::EINVAL),
                _ => (),
            }
        }

        let data = match spec {
            MakeFileSpec::Fifo => RamFsData::Fifo,
            MakeFileSpec::CharDev(dev) => RamFsData::CharDev(dev),
            MakeFileSpec::Directory => RamFsData::Directory(BTreeMap::new()),
            MakeFileSpec::BlockDev(dev) => RamFsData::BlockDev(dev),
            MakeFileSpec::Regular => RamFsData::Regular(Vec::new()),
            MakeFileSpec::Symlink(items) => RamFsData::Symlink(items.into()),
            MakeFileSpec::UnixSocket => RamFsData::UnixSocket,
        };
        let now = Timespec::now();
        let ino = ramfs.ino_ctr.fetch_add(1, Ordering::Relaxed);
        let new_inode = Arc::try_new(UnsafeCell::new(RamINode {
            data,
            ino,
            links: Spinlock::new(1),
            size: AtomicUsize::new(0),
            atim: AtomicTimespec::new(now),
            mtim: AtomicTimespec::new(now),
            ctim: AtomicTimespec::new(now),
        }))?;

        let ops = Box::try_new(RamVNode {
            inode: new_inode.clone(),
        })?;

        ramfs.inodes.lock()?.insert(ino, new_inode.clone());

        let dirent = Dirent {
            ino,
            type_: unsafe { new_inode.as_ref_unchecked() }.data.node_type(),
            name: name.into(),
            dirent_disk_off: 0,
            dirent_off: 0,
        };
        directory.insert(name.into(), dirent.clone());

        Ok((dirent, Box::<dyn VNodeOps>::from(ops)))
    }

    fn rename(
        &mut self,
        _arc_self: &Arc<VNode>,
        old_name: &[u8],
        new_name: &[u8],
    ) -> EResult<Dirent> {
        let inode = unsafe { self.inode.as_mut_unchecked() };
        let directory = inode.data.as_directory_mut().ok_or(Errno::EINVAL)?;

        if old_name == new_name {
            return directory.get(old_name).cloned().ok_or(Errno::ENOENT);
        }

        if let Some(dirent) = directory.get(old_name) {
            let mut dirent = dirent.clone();
            dirent.name = new_name.into();
            directory
                .try_insert(new_name.into(), dirent.clone())
                .map_err(|_| Errno::EEXIST)?;
            directory.remove(old_name).unwrap();
            Ok(dirent)
        } else {
            Err(Errno::ENOENT)
        }
    }

    fn readlink(&self, _arc_self: &Arc<VNode>) -> EResult<Box<[u8]>> {
        let inode = unsafe { self.inode.as_ref_unchecked() };
        inode.data.as_symlink().ok_or(Errno::EINVAL).cloned()
    }

    fn stat(&self, _arc_self: &Arc<VNode>) -> EResult<Stat> {
        let inode = unsafe { self.inode.as_ref_unchecked() };
        let size = inode.size.load(Ordering::Relaxed);
        assert!(unsafe { cpu::irq::disable() });
        let nlink = *inode.links.lock_shared();
        unsafe { cpu::irq::enable() };
        Ok(Stat {
            dev: 0,
            ino: 0,
            mode: NodeMode {
                type_: inode.data.node_type(),
                others: 7,
                group: 7,
                owner: 7,
                suid: false,
                sgid: false,
                sticky: false,
            }
            .into_u16(),
            nlink,
            uid: 0,
            gid: 0,
            rdev: 0,
            size: size as u64,
            blksize: 1,
            blocks: (size / 512) as u64,
            atim: inode.atim.load(),
            mtim: inode.mtim.load(),
            ctim: inode.ctim.load(),
        })
    }

    fn get_inode(&self) -> u64 {
        let inode = unsafe { self.inode.as_ref_unchecked() };
        inode.ino
    }

    fn get_size(&self, _arc_self: &Arc<VNode>) -> u64 {
        let inode = unsafe { self.inode.as_ref_unchecked() };
        inode.size.load(Ordering::Relaxed) as u64
    }

    fn get_type(&self, _arc_self: &Arc<VNode>) -> NodeType {
        let inode = unsafe { self.inode.as_ref_unchecked() };
        inode.data.node_type()
    }

    fn sync(&self, _arc_self: &Arc<VNode>) -> EResult<()> {
        Ok(())
    }
}

/// The driver struct for [`RamFS`].
struct RamFsDriver {
    allows_devfiles: bool,
}

impl VfsDriver for RamFsDriver {
    fn detect(&self, _media: &Media) -> EResult<bool> {
        Ok(false)
    }

    fn mount(&self, media: Option<Media>, mflags: MFlags) -> EResult<Box<dyn VfsOps>> {
        if mflags & mflags::READ_ONLY != 0 {
            logkf!(
                LogLevel::Error,
                "It doesn't make sense to mount an empty RamFS as READ_ONLY"
            );
            return Err(Errno::EINVAL);
        }
        if let Some(_) = media {
            logkf!(LogLevel::Error, "RamFS does not use media");
            return Err(Errno::EINVAL);
        }
        Ok(Box::<dyn VfsOps>::from(Box::try_new(RamFs::new(
            self.allows_devfiles,
        )?)?))
    }
}

fn register_ramfs() {
    FSDRIVERS.unintr_lock().insert(
        "ramfs".into(),
        Box::new(RamFsDriver {
            allows_devfiles: false,
        }),
    );
    FSDRIVERS.unintr_lock().insert(
        "devtmpfs".into(),
        Box::new(RamFsDriver {
            allows_devfiles: true,
        }),
    );
}

register_kmodule!(ramfs, [1, 0, 0], register_ramfs);
