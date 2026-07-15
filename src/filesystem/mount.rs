// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    debug_assert,
    ops::Range,
    sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering},
};

use alloc::{boxed::Box, collections::btree_map::BTreeMap, string::String, sync::Arc, vec::Vec};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    dev2::{self, Device, class::block::BlockDevice},
    filesystem::{
        self, DentCache, DentCacheDir, DentCacheType, Dirent, File, InodeType,
        media::Media,
        oflags,
        vfs::{VNode, VNodeMtxInner, Vfs, VfsDriver},
    },
    kernel::sync::mutex::{Mutex, SharedMutexGuard},
};

/// Filesystem is read-only.
pub const READ_ONLY: u32 = 0x0000_0001;
/// Do not follow symbolic links.
pub const NOFOLLOW: u32 = 0x0000_0020;
/// Try to cancel pending I/O operations; only supported on certain filesystems.
pub const FORCE: u32 = 0x0001_0000;
/// Lazily unmount; remove the filesystem from the tree now, and wait with unmount until open handles are closed.
pub const DETACH: u32 = 0x0002_0000;

/// Key type used for filesystem by media table.
#[derive(Clone)]
struct MediaKey {
    /// Device that the media references.
    device: Arc<dyn BlockDevice>,
    /// Partition offset.
    offset: Option<Range<u64>>,
}

impl MediaKey {
    fn new(media: &Media) -> Option<Self> {
        let device = media.device()?;
        Some(MediaKey {
            offset: (media.offset != 0
                || media.offset != device.block_count() << device.block_size_exp())
            .then_some(media.offset..media.offset + media.size),
            device,
        })
    }
}

impl PartialEq for MediaKey {
    fn eq(&self, other: &Self) -> bool {
        (&*self.device as &dyn Device).id() == (&*other.device as &dyn Device).id()
            && self.offset == other.offset
    }
}
impl Eq for MediaKey {}
impl PartialOrd for MediaKey {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for MediaKey {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        match (&*self.device as &dyn Device)
            .id()
            .cmp(&(&*other.device as &dyn Device).id())
        {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        let self_off = self.offset.clone().map(|x| x.start).unwrap_or(0);
        let other_off = other.offset.clone().map(|x| x.start).unwrap_or(0);
        self_off.cmp(&other_off)
    }
}

/// Table of mounted filesystems.
pub(super) struct MountTable {
    fs_by_media: BTreeMap<MediaKey, Arc<Vfs>>,
    fs_by_mount: BTreeMap<Box<[u8]>, Arc<Vfs>>,
}

/// The currently mounted root filesystem.
pub(super) static ROOT_FS: Mutex<Option<Arc<Vfs>>> = Mutex::new(None);

/// Table of mounted filesystems.
pub(super) static MOUNT_TABLE: Mutex<MountTable> = Mutex::new(MountTable {
    fs_by_media: BTreeMap::new(),
    fs_by_mount: BTreeMap::new(),
});

/// Helper function that gets the root directory handle.
pub(super) fn root_vnode_unlocked(guard: &MountTable) -> EResult<Arc<VNode>> {
    debug_assert!(core::ptr::addr_eq(guard, unsafe { MOUNT_TABLE.data() }));
    if let Some(fs) = &*ROOT_FS.lock_shared()? {
        Ok(fs.root())
    } else {
        logkf!(
            LogLevel::Warning,
            "Filesystem op run without a filesystem mounted"
        );
        Err(Errno::EAGAIN)
    }
}

/// Helper function that gets the root directory handle.
pub(super) fn root_vnode() -> EResult<Arc<VNode>> {
    root_vnode_unlocked(&*MOUNT_TABLE.lock_shared()?)
}

/// Detect the filesystem on a medium.
fn detect<'a>(
    media: &Media,
    drivers: &'a BTreeMap<String, Box<dyn VfsDriver>>,
) -> EResult<&'a str> {
    for ent in drivers {
        if ent.1.detect(media)? {
            return Ok(&*ent.0);
        }
    }
    logkf!(LogLevel::Error, "Cannot detect filesystem type");
    return Err(Errno::ENOTSUP);
}

/// Helper function that prepares a standalone [`Vfs`] to be used by [`mount`].
fn create_vfs(
    drivers: &SharedMutexGuard<'_, BTreeMap<String, Box<dyn VfsDriver>>>,
    mountpoint: Option<Arc<VNode>>,
    type_: &str,
    media: Option<Media>,
    mflags: u32,
) -> EResult<Arc<Vfs>> {
    let driver = if let Some(x) = drivers.get(type_) {
        x
    } else {
        logkf!(LogLevel::Error, "No such filesystem driver: {}", type_);
        return Err(Errno::ENOTSUP);
    };

    let vfs_ops = driver.mount(media, mflags)?;
    let block_size_exp = vfs_ops.block_size_exp();

    let vfs = Arc::try_new(Vfs {
        flags: AtomicU32::new(vfs_ops.read_only() as u32 * READ_ONLY),
        ops: Mutex::new(vfs_ops),
        vnodes: Mutex::new(BTreeMap::new()),
        root: UnsafeCell::new(None),
        mountpoint,
        next_fake_ino: AtomicU64::new(1),
        block_size_exp,
    })
    .unwrap();

    let root_ops = vfs.ops.unintr_lock_shared().open_root(&vfs)?;
    let root_ino = if vfs.ops.unintr_lock_shared().uses_inodes() {
        root_ops.get_inode()
    } else {
        vfs.next_fake_ino.fetch_add(1, Ordering::Relaxed)
    };

    let dentcache = Arc::try_new(DentCache {
        type_: DentCacheType::Directory(Mutex::new(DentCacheDir::EMPTY)),
        vfs: vfs.clone(),
        parent: None,
        vnode: Mutex::new(None),
        dirent: Dirent {
            ino: root_ino,
            type_: InodeType::Directory,
            name: Box::try_new(*b"/")?,
            dirent_off: 0,
            dirent_disk_off: 0,
        },
    })?;

    let root = Arc::new(VNode {
        mtx: Mutex::new(VNodeMtxInner {
            ops: root_ops,
            flags: 0,
            dentcache: Some(dentcache),
        }),
        ino: root_ino,
        vfs: vfs.clone(),
        type_: InodeType::Directory,
        fifo: None,
        pagecache: None,
        mappings: Mutex::new(Vec::new()),
        denywrite: AtomicI32::new(0),
    });
    vfs.vnodes
        .unintr_lock()
        .insert(root.ino, Arc::downgrade(&root));
    unsafe { *vfs.root.as_mut_unchecked() = Some(root) };

    Ok(vfs)
}

/// Mount a new filesystem.
pub fn mount(
    at: Option<&dyn File>,
    path: &[u8],
    type_: Option<&str>,
    media: Option<Media>,
    mflags: u32,
) -> EResult<()> {
    // Determine filesystem type.
    let drivers = filesystem::FSDRIVERS.lock_shared()?;
    let type_ = if let Some(x) = type_ {
        x
    } else if let Some(media) = &media {
        detect(media, &drivers)?
    } else {
        logkf!(LogLevel::Error, "Neither type nor media specified to mount");
        return Err(Errno::EINVAL);
    };

    // Lock mounts table while other mounting logic runs.
    let mut mounts = MOUNT_TABLE.lock()?;
    let media_key = try { MediaKey::new(media.as_ref()?)? };

    // Cloning mounts is currently unsupported.
    if let Some(media_key) = &media_key
        && mounts.fs_by_media.contains_key(media_key)
    {
        logkf!(
            LogLevel::Warning,
            "TODO: Cloning mounts is not supported yet"
        );
        return Err(Errno::EBUSY);
    }

    // If the mounts table is empty (there is no root VFS), this must be mounted at `/`.
    if mounts.fs_by_mount.len() == 0 {
        if path != b"/" {
            logkf!(LogLevel::Error, "/ needs to be mounted first");
            return Err(Errno::ENOENT);
        }
        let vfs = create_vfs(&drivers, None, type_, media, mflags)?;
        mounts.fs_by_mount.insert((*b"/").into(), vfs.clone());
        if let Some(media_key) = media_key {
            mounts.fs_by_media.insert(media_key, vfs.clone());
        }
        *ROOT_FS.unintr_lock() = Some(vfs);
        return Ok(());
    }

    // Get the directory that is requested for the mountpoint.
    let orig_at = at;
    let at = filesystem::at_vnode_unlocked(at, &mounts)?;
    let cache = filesystem::walk_unlocked(
        at.mtx
            .lock_shared()?
            .dentcache
            .clone()
            .ok_or(Errno::ENOTDIR)?,
        path,
        true,
        &mounts,
    )?
    .follow_mounts();
    // Lock it so no modifications can happen while mounting there.
    let cache_dir = cache.type_.as_dir().ok_or(Errno::ENOTDIR)?;
    let mut cache_guard = cache_dir.lock()?;

    if cache_guard.children.len() != 0 {
        // Mountpoint must be empty.
        logkf!(LogLevel::Error, "Mountpoint root isn't empty");
        return Err(Errno::ENOTEMPTY);
    } else if cache.is_vfs_root() {
        // Cannot stack mounts.
        logkf!(
            LogLevel::Warning,
            "TODO: Stacked mounts are not supported yet"
        );
        return Err(Errno::ENOTSUP);
    }

    // Create and insert VFS.
    let vfs = create_vfs(&drivers, Some(cache.open_vnode()?), type_, media, mflags)?;
    mounts
        .fs_by_mount
        .insert(cache.realpath()?.into(), vfs.clone());
    if let Some(media_key) = media_key {
        mounts.fs_by_media.insert(media_key, vfs.clone());
    }
    cache_guard.mounted = Some(vfs);

    drop(cache_guard);
    drop(mounts);
    drop(drivers);

    // Notify device subsystem.
    if let EResult::Err(x) = try {
        let vfs_root_dir = filesystem::open(orig_at, path, oflags::DIR_ONLY | oflags::READ_ONLY)?;
        dev2::node::populate(&*vfs_root_dir)?;
    } {
        logkf!(LogLevel::Warning, "Failed to populate devtmpfs: {}", x);
    }

    Ok(())
}

/// Unmount an existing filesystem by mountpoint or device.
pub fn umount(at: Option<&dyn File>, path: &[u8], flags: u32) -> EResult<()> {
    // Open the media / VFS root VNode.
    let target = filesystem::open(at, path, flags & oflags::NOFOLLOW)?
        .get_vnode()
        .unwrap();

    // Now, lock the mount table; this will inhibit any new VNodes from being opened.
    let mut mount_table = MOUNT_TABLE.lock()?;
    // Get the target VFS from this VNode.
    let mut vfs = target.follow_mounts().is_vfs_root();
    if vfs.is_none() {
        let ops = &target.mtx.lock_shared()?.ops;
        vfs = try {
            let device = ops.get_device(&target)?.try_as_arc()?;
            let offset = ops.get_part_offset(&target);
            let media_key = MediaKey { device, offset };
            mount_table.fs_by_media.get(&media_key).cloned()?
        };
    }
    let vfs = vfs.ok_or(Errno::ENOENT)?;

    // Assert that no files are open; only the root dir VNode should be present with refcount 2.
    if flags & DETACH == 0 {
        let root = unsafe { vfs.root.as_ref_unchecked() }.as_ref().unwrap();
        for weak in vfs.vnodes.lock_shared()?.values() {
            if weak
                .upgrade()
                .map(|arc| !Arc::ptr_eq(root, &arc))
                .unwrap_or(false)
            {
                return Err(Errno::EBUSY);
            }
        }
        debug_assert!(Arc::strong_count(root) >= 2);
        if Arc::strong_count(root) > 2 {
            return Err(Errno::EBUSY);
        }
    }

    // OK to unmount, remove from mount table.
    if let Some(key) = try { MediaKey::new(vfs.ops.unintr_lock_shared().media()?)? } {
        mount_table.fs_by_media.remove(&key).unwrap();
    }
    let mountpoint = if let Some(vnode) = vfs.mountpoint.clone() {
        let dentcache = vnode.mtx.unintr_lock_shared().dentcache.clone().unwrap();
        dentcache.type_.as_dir().unwrap().unintr_lock().mounted = None;
        &*(dentcache.realpath()?)
    } else {
        b"/"
    };
    mount_table.fs_by_mount.remove(mountpoint).unwrap();
    // Remove root VNode from it to break circular reference.
    *unsafe { vfs.root.as_mut_unchecked() } = None;

    Ok(())
}
