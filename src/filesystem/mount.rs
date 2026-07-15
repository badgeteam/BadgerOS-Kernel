// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ops::Range,
    sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering},
    todo,
};

use alloc::{boxed::Box, collections::btree_map::BTreeMap, string::String, sync::Arc, vec::Vec};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    dev2::{Device, class::block::BlockDevice},
    filesystem::{
        self, DentCache, DentCacheDir, DentCacheType, Dirent, File, InodeType, VfsLoc,
        media::Media,
        vfs::{VNode, VNodeMtxInner, Vfs, VfsDriver},
    },
    kernel::sync::mutex::Mutex,
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

/// Used to track the mountpoint of FSes mounted twice or more.
pub(super) struct Mount {
    /// Mountpoint, or [`None`] for the root mount.
    pub(super) parent: Option<VfsLoc>,
    /// VNode of `vfs` that is the root of this mount.
    pub(super) root: Arc<VNode>,
    /// Mounted filesystem data.
    pub(super) vfs: Arc<Vfs>,
}

/// Table of mounted filesystems.
pub(super) struct MountTable {
    /// Filesystems with block devices.
    by_media: BTreeMap<MediaKey, (Arc<Vfs>, Arc<VNode>)>,
    /// List of all filesystems.
    all: Vec<Arc<Mount>>,
    /// The root filesystem.
    root: Option<Arc<Mount>>,
}

/// Table of mounted filesystems.
pub(super) static MOUNT_TABLE: Mutex<MountTable> = Mutex::new(MountTable {
    by_media: BTreeMap::new(),
    all: Vec::new(),
    root: None,
});

/// Helper function that gets the root directory handle.
pub(super) fn root_loc_unlocked(guard: &MountTable) -> EResult<VfsLoc> {
    if let Some(mount) = guard.root.clone() {
        let vnode = mount.root.clone();
        Ok(VfsLoc { vnode, mount })
    } else {
        logkf!(
            LogLevel::Warning,
            "Filesystem op run without a filesystem mounted"
        );
        Err(Errno::EAGAIN)
    }
}

/// Helper function that gets the root directory handle.
pub(super) fn root_loc() -> EResult<VfsLoc> {
    root_loc_unlocked(&*MOUNT_TABLE.lock_shared()?)
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
    drivers: &BTreeMap<String, Box<dyn VfsDriver>>,
    type_: &str,
    media: Option<Media>,
    mflags: u32,
) -> EResult<(Arc<Vfs>, Arc<VNode>)> {
    let driver = if let Some(x) = drivers.get(type_) {
        x
    } else {
        logkf!(LogLevel::Error, "No such filesystem driver: {}", type_);
        return Err(Errno::ENOTSUP);
    };

    let ops = driver.mount(media, mflags)?;
    let block_size_exp = ops.block_size_exp();

    let vfs = Arc::try_new(Vfs {
        flags: AtomicU32::new(ops.read_only() as u32 * READ_ONLY),
        ops,
        vnodes: Mutex::new(BTreeMap::new()),
        next_fake_ino: AtomicU64::new(1),
        block_size_exp,
    })
    .unwrap();

    let root_ops = vfs.ops.open_root(&vfs)?;
    let root_ino = if vfs.ops.uses_inodes() {
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

    Ok((vfs, root))
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

    // Locate mount point.
    let parent;
    if mounts.root.is_some() {
        let loc = filesystem::vfs_loc_unlocked(at, &mounts)?;
        let res = filesystem::walk_unlocked(
            loc.clone().to_cache()?,
            path,
            mflags & NOFOLLOW == 0,
            &mounts,
        )?;
        let vnode = res.cache.open_vnode()?;
        parent = Some(VfsLoc {
            vnode,
            mount: loc.mount.clone(),
        });
    } else {
        if path != b"/" {
            logkf!(LogLevel::Error, "First FS must be mounted at /");
            return Err(Errno::EINVAL);
        }
        parent = None;
    }

    // Get or create VFS.
    let vfs;
    let root;
    if let Some(media_key) = &media_key
        && let Some(existing) = mounts.by_media.get(media_key)
    {
        (vfs, root) = existing.clone();
    } else {
        (vfs, root) = create_vfs(&drivers, type_, media, mflags)?;
    }

    let mount = Arc::new(Mount { parent, root, vfs });
    if mounts.root.is_none() {
        mounts.root = Some(mount.clone());
    }
    mounts.all.push(mount);

    Ok(())
}

/// Unmount an existing filesystem by mountpoint or device.
pub fn umount(at: Option<&dyn File>, path: &[u8], flags: u32) -> EResult<()> {
    logkf!(LogLevel::Warning, "TODO: mount::umount()");
    Err(Errno::ENOSYS)
}
