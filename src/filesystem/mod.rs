// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    fmt::{Debug, Write},
    ops::Range,
    panic, ptr, str,
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
};

use access::Access;
use alloc::{boxed::Box, collections::btree_map::BTreeMap, string::String, sync::Arc, vec::Vec};
use bytemuck::{AnyBitPattern, NoUninit};
use device::{BlockDevFile, CharDevFile};
use linkflags::LinkFlags;
use media::Media;
use oflags::OFlags;
use sysimpl::DentBuffer;
use vfs::{
    DentCache, DentCacheDir, DentCacheType, FlagsAndOffset, VNode, Vfs, VfsDriver, VfsFile,
    mflags::MFlags,
};

use crate::{
    LogLevel,
    badgelib::time::Timespec,
    bindings::{
        device::{
            BaseDevice, HasBaseDevice,
            class::{block::BlockDevice, char::CharDevice},
        },
        error::{EResult, Errno},
        raw::{errno_t, file_t},
    },
    filesystem::{
        c_api::ref_as_file,
        fifo::{Fifo, FifoShared},
        vfs::{VNodeMtxInner, mflags, vnflags},
    },
    kernel::sync::mutex::{Mutex, SharedMutexGuard},
    mem::vmm::{memobject::MemObject, pagecache::PageCache},
    process::{
        uapi::stat::stat,
        usercopy::{UserSlice, UserSliceMut},
    },
};

pub mod c_api;
pub mod device;
pub mod ext2;
pub mod fatfs;
pub mod fifo;
pub mod media;
pub mod mount_root;
pub mod partition;
pub mod ramfs;
pub mod sysimpl;
pub mod test;
pub mod vfs;

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
/// Seek modes to give to [`File::seek`].
pub enum SeekMode {
    /// Set absolute position.
    Set = 0,
    /// Set relative to current position.
    Cur = 1,
    /// Set relative to end of file.
    End = 2,
}

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
/// What operation to perform with [`vfs::VNodeOps::unlink`].
pub enum UnlinkMode {
    /// Fail if target is a directory.
    FileOnly,
    /// Fail if the target is not a directory.
    DirOnly,
    /// Both files and directories may be unlinked.
    Any,
}

#[rustfmt::skip]
pub mod access {
    pub type Access = u8;
    pub const READ:  u8 = 0b001;
    pub const WRITE: u8 = 0b010;
    pub const EXEC:  u8 = 0b100;
}

#[rustfmt::skip]
pub mod mode {
    pub type Mode = u16;
    /// bit mask for the file type bit field
    pub const S_IFMT:   u16 = 0o170000;
    /// socket
    pub const S_IFSOCK: u16 = 0o140000;
    /// symbolic link
    pub const S_IFLNK:  u16 = 0o120000;
    /// regular file
    pub const S_IFREG:  u16 = 0o100000;
    /// block device
    pub const S_IFBLK:  u16 = 0o060000;
    /// directory
    pub const S_IFDIR:  u16 = 0o040000;
    /// character device
    pub const S_IFCHR:  u16 = 0o020000;
    /// FIFO
    pub const S_IFIFO:  u16 = 0o010000;
}

#[derive(Clone, Copy, PartialEq, Eq)]
/// Inode mode.
pub struct NodeMode {
    pub type_: NodeType,
    pub others: Access,
    pub group: Access,
    pub owner: Access,
    pub suid: bool,
    pub sgid: bool,
    pub sticky: bool,
}

impl NodeMode {
    /// Convert into the format for the posix `struct stat` `st_mode` field.
    pub const fn into_u16(self) -> u16 {
        let mode = match self.type_ {
            NodeType::Unknown => 0,
            NodeType::Fifo => mode::S_IFIFO,
            NodeType::CharDev => mode::S_IFCHR,
            NodeType::Directory => mode::S_IFDIR,
            NodeType::BlockDev => mode::S_IFBLK,
            NodeType::Regular => mode::S_IFREG,
            NodeType::Symlink => mode::S_IFLNK,
            NodeType::UnixSocket => mode::S_IFSOCK,
        };
        mode + (self.others as u16) * 0o0001
            + (self.group as u16) * 0o0010
            + (self.owner as u16) * 0o0100
            + self.suid as u16 * 0o4000
            + self.sgid as u16 * 0o2000
            + self.sticky as u16 * 0o1000
    }

    /// Convert from the format for the posix `struct stat` `st_mode` field.
    pub const fn from_u16(value: u16) -> Self {
        let type_ = match value & mode::S_IFMT {
            mode::S_IFIFO => NodeType::Fifo,
            mode::S_IFCHR => NodeType::CharDev,
            mode::S_IFDIR => NodeType::Directory,
            mode::S_IFBLK => NodeType::BlockDev,
            mode::S_IFREG => NodeType::Regular,
            mode::S_IFLNK => NodeType::Symlink,
            mode::S_IFSOCK => NodeType::UnixSocket,
            _ => NodeType::Unknown,
        };
        Self {
            type_,
            others: (value & 0o007) as u8,
            group: ((value & 0o070) >> 3) as u8,
            owner: ((value & 0o700) >> 3) as u8,
            suid: value & 0o4000 != 0,
            sgid: value & 0o2000 != 0,
            sticky: value & 0o1000 != 0,
        }
    }
}

impl Into<u16> for NodeMode {
    fn into(self) -> u16 {
        self.into_u16()
    }
}

impl From<u16> for NodeMode {
    fn from(value: u16) -> Self {
        Self::from_u16(value)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
/// Inode statistics obtained from [`File::stat`].
pub struct Stat {
    /// ID and class of device containing file.
    /// Upper 32 bits are ID, lower 32 are class.
    pub dev: u64,
    /// Inode number.
    pub ino: u64,
    /// File type and mode flags
    pub mode: u16,
    /// Number of hard links.
    pub nlink: u16,
    /// Owner user ID.
    pub uid: u16,
    /// Owner group ID.
    pub gid: u16,
    /// ID of device for device special files.
    /// Upper 32 bits are ID, lower 32 are class.
    pub rdev: u64,
    /// Byte size of this file.
    pub size: u64,
    /// Block size for filesystem I/O.
    pub blksize: u64,
    /// Number of 512 byte blocks allocated (represents actual used disk space).
    pub blocks: u64,
    /// Time of last access. On BadgerOS, only updated when modified or created.
    pub atim: Timespec,
    /// Time of last modification.
    pub mtim: Timespec,
    /// Time of last status change.
    pub ctim: Timespec,
}

impl Into<stat> for Stat {
    fn into(self) -> stat {
        stat {
            st_dev: self.dev,
            st_ino: self.ino as i64,
            st_mode: self.mode as u32,
            st_nlink: self.nlink as u64,
            st_uid: self.uid as u32,
            st_gid: self.gid as u32,
            st_rdev: self.rdev,
            st_size: self.size as i64,
            st_blksize: self.blksize,
            st_blocks: self.blocks as i64,
            st_atim: self.atim.into(),
            st_mtim: self.mtim.into(),
            st_ctim: self.ctim.into(),
        }
    }
}

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
/// Types of file recognised by [`Dirent`].
pub enum NodeType {
    #[default]
    /// Unknown file type.
    Unknown,
    /// Named pipe.
    Fifo,
    /// Character device.
    CharDev,
    /// Directory.
    Directory,
    /// Block device.
    BlockDev,
    /// Regular file.
    Regular,
    /// Symbolic link.
    Symlink,
    /// UNIX domain socket.
    UnixSocket,
}

#[derive(Clone, Default)]
/// An abstract directory entry obtained from [`VNodeOps::find_dirent`].
pub struct Dirent {
    /// Inode number.
    pub ino: u64,
    /// Type of entry this is.
    pub type_: NodeType,
    /// File name:
    pub name: Box<[u8]>,
    /// On-disk position of the dirent.
    pub dirent_disk_off: u64,
    /// In-directory position of the dirent.
    pub dirent_off: u64,
}

impl Debug for Dirent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Dirent")
            .field("ino", &self.ino)
            .field("type_", &self.type_)
            .field("name", &{
                struct ByteStr<'a>(&'a [u8]);
                impl<'a> core::fmt::Debug for ByteStr<'a> {
                    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                        f.write_str("b\"")?;
                        for &b in self.0 {
                            match b {
                                b'\\' => f.write_str("\\\\")?,
                                b'"' => f.write_str("\\\"")?,
                                0x20..=0x7E => f.write_char(b as char)?,
                                _ => write!(f, "\\x{:02x}", b)?,
                            }
                        }
                        f.write_str("\"")
                    }
                }
                ByteStr(&self.name)
            })
            .field("dirent_off", &self.dirent_off)
            .finish()
    }
}

/// Handle to an open file. Dropping it closes the file.
pub trait File: Sync {
    /// Get file mode and access flags.
    fn get_flags(&self) -> u32;
    /// Try to set file mode and access flags.
    fn set_flags(&self, newfl: u32) -> EResult<()>;
    /// Succeed if this is a TTY, fail otherwise.
    fn isatty(&self) -> EResult<()> {
        Err(Errno::ENOTTY)
    }
    /// Read directory entries into the buffer.
    fn get_dirents(&self, buffer: &mut DentBuffer<'_>) -> EResult<()>;
    /// Get the memory object for this file, if any.
    fn get_memobject(&self) -> Option<Arc<dyn MemObject>> {
        None
    }
    /// Get the device that this file represents, if any.
    fn get_device(&self) -> Option<BaseDevice> {
        None
    }
    /// Get the partition offset and size that this file represents, if any.
    fn get_part_offset(&self) -> Option<Range<u64>> {
        None
    }
    /// Get the stat info for this file's inode.
    fn stat(&self) -> EResult<Stat>;
    /// Get the position in the file.
    fn tell(&self) -> EResult<u64>;
    /// Change the position in the file.
    fn seek(&self, mode: SeekMode, offset: i64) -> EResult<u64>;
    /// Write bytes to this file.
    fn write(&self, wdata: UserSlice<'_, u8>) -> EResult<usize>;
    /// Read bytes from this file.
    fn read(&self, rdata: UserSliceMut<'_, u8>) -> EResult<usize>;
    /// Resize the file to a new length.
    fn resize(&self, size: u64) -> EResult<()>;
    /// Sync the underlying caches to disk.
    fn sync(&self) -> EResult<()>;
    /// Get the underlying vnode (if it exists).
    fn get_vnode(&self) -> Option<Arc<VNode>>;
}

impl dyn File + '_ {
    /// Write bytes to this file.
    #[inline(always)]
    pub fn writek(&self, wdata: &[u8]) -> EResult<usize> {
        self.write(UserSlice::new_kernel(wdata))
    }

    /// Read bytes from this file.
    #[inline(always)]
    pub fn readk(&self, rdata: &mut [u8]) -> EResult<usize> {
        self.read(UserSliceMut::new_kernel_mut(rdata))
    }

    /// Helper function for [`Self::seek`] with [`SeekMode::Set`] that will return `seek_error` if the desired position is not reached.
    pub fn seek_strong(&self, offset: u64, seek_error: Errno) -> EResult<()> {
        let pos = self.seek(SeekMode::Set, offset as i64)?;
        if pos != offset {
            return Err(seek_error);
        }
        Ok(())
    }

    /// Write the binary representation of a POD object.
    #[inline(always)]
    pub fn write_pod<T: NoUninit>(&self, wdata: &T) -> EResult<usize> {
        self.write(UserSlice::new_kernel(bytemuck::bytes_of(wdata)))
    }

    /// Read a POD object from its binary representation.
    #[inline(always)]
    pub fn read_pod<T: NoUninit + AnyBitPattern>(&self, short_error: Errno) -> EResult<T> {
        let mut rdata: T = unsafe { core::mem::zeroed() };
        if self.read(UserSliceMut::new_kernel_mut(bytemuck::bytes_of_mut(
            &mut rdata,
        )))? != size_of::<T>()
        {
            return Err(short_error);
        }
        Ok(rdata)
    }
}

#[derive(Clone)]
/// Specifies how a file is to be created.
pub enum MakeFileSpec<'a> {
    /// Named pipe.
    Fifo,
    /// Character device.
    CharDev(CharDevice),
    /// Directory.
    Directory,
    /// Block device.
    BlockDev((BlockDevice, Option<Range<u64>>)),
    /// Regular file.
    Regular,
    /// Symbolic link.
    Symlink(&'a [u8]),
    /// UNIX domain socket.
    UnixSocket,
}

impl MakeFileSpec<'_> {
    pub fn node_type(&self) -> NodeType {
        match self {
            MakeFileSpec::Fifo => NodeType::Fifo,
            MakeFileSpec::CharDev(_) => NodeType::CharDev,
            MakeFileSpec::Directory => NodeType::Directory,
            MakeFileSpec::BlockDev(_) => NodeType::BlockDev,
            MakeFileSpec::Regular => NodeType::Regular,
            MakeFileSpec::Symlink(_) => NodeType::Symlink,
            MakeFileSpec::UnixSocket => NodeType::UnixSocket,
        }
    }
}

#[rustfmt::skip]
pub mod oflags {
    /// Type to use for file opening flags.
    pub type OFlags = u32;
    /// Allows for reading the file.
    pub const READ_ONLY:  u32 = 0x0000_0001;
    /// Allows for writing the file.
    pub const WRITE_ONLY: u32 = 0x0000_0002;
    /// Allows for both reading and writing.
    pub const READ_WRITE: u32 = 0x0000_0003;
    /// Makes writing work in append mode.
    pub const APPEND:     u32 = 0x0000_0004;
    /// Fail if the target is a directory.
    pub const FILE_ONLY:  u32 = 0x0000_0008;
    /// Fail if the target is not a directory.
    pub const DIR_ONLY:   u32 = 0x0000_0010;
    /// Do not follow the last symlink.
    pub const NOFOLLOW:   u32 = 0x0000_0020;
    /// Create the file if it does not exist.
    pub const CREATE:     u32 = 0x0000_0040;
    /// Fail if the file exists already.
    pub const EXCLUSIVE:  u32 = 0x0000_0080;
    /// Truncate the file on open.
    pub const TRUNCATE:   u32 = 0x0000_0100;
    /// Use non-blocking I/O.
    pub const NONBLOCK:   u32 = 0x0000_0200;
    /// TODO: Delete file after last reference to it is dropped.
    pub const TMPFILE:    u32 = 0x0000_0400;
    /// Dup: Use next available instead of overwriting (fncntl(F_DUPFD)-style).
    pub const DUP_FCNTL:  u32 = 0x0000_0800;
}

#[rustfmt::skip]
pub mod linkflags {
    pub type LinkFlags = u32;
    /// Follow symlinks for old path on [`super::link`] and [`super::rename`].
    pub const FOLLOW_LINKS: u32 = 0x0000_0001;
}

#[rustfmt::skip]
pub mod renameflags {
    /// Exchange oldpath and newpath instead of replacing newpath.
    pub const EXCHANGE: u32 = 1;
    /// Fail if newpath already exists.
    pub const NOREPLACE: u32 = 2;
}

/// The maximum number of symlinks followed.
pub const LINK_MAX: usize = 32;
/// The maximum path length.
pub const PATH_MAX: usize = 4096;
/// The maximum filename length.
pub const NAME_MAX: usize = 255;

/// Key type used for filesystem by media table.
#[derive(Clone)]
struct MediaKey {
    /// Device that the media references.
    device: BlockDevice,
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
        self.device.id() == other.device.id() && self.offset == other.offset
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
        match self.device.id().cmp(&other.device.id()) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        let self_off = self.offset.clone().map(|x| x.start).unwrap_or(0);
        let other_off = other.offset.clone().map(|x| x.start).unwrap_or(0);
        self_off.cmp(&other_off)
    }
}

/// Table of mounted filesystems.
struct MountTable {
    fs_by_media: BTreeMap<MediaKey, Arc<Vfs>>,
    fs_by_mount: BTreeMap<Box<[u8]>, Arc<Vfs>>,
}

/// The currently mounted root filesystem.
static ROOT_FS: Mutex<Option<Arc<Vfs>>> = Mutex::new(None);

/// Table of mounted filesystems.
static MOUNT_TABLE: Mutex<MountTable> = Mutex::new(MountTable {
    fs_by_media: BTreeMap::new(),
    fs_by_mount: BTreeMap::new(),
});

/// Table of filesystem drivers.
pub static FSDRIVERS: Mutex<BTreeMap<String, Box<dyn VfsDriver>>> = Mutex::new(BTreeMap::new());

/// Helper function that gets the root directory handle.
fn root_vnode_unlocked(guard: &MountTable) -> EResult<Arc<VNode>> {
    debug_assert!(ptr::addr_eq(guard, unsafe { MOUNT_TABLE.data() }));
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
fn root_vnode() -> EResult<Arc<VNode>> {
    root_vnode_unlocked(&*MOUNT_TABLE.lock_shared()?)
}

/// Helper function that gets the VNode for `at` parameters.
fn at_vnode_unlocked(at: Option<&dyn File>, guard: &MountTable) -> EResult<Arc<VNode>> {
    debug_assert!(ptr::addr_eq(guard, unsafe { MOUNT_TABLE.data() }));
    match at {
        Some(x) => x.get_vnode().ok_or(Errno::ENOTDIR),
        None => root_vnode_unlocked(guard),
    }
}

/// Helper function that gets the VNode for `at` parameters.
fn at_vnode(at: Option<&dyn File>) -> EResult<Arc<VNode>> {
    at_vnode_unlocked(at, &*MOUNT_TABLE.lock_shared()?)
}

/// Walk down the filesystem to a certain path.
fn walk_unlocked(
    mut at: Arc<DentCache>,
    path: &[u8],
    follow_last_symlink: bool,
    guard: &MountTable,
) -> EResult<Arc<DentCache>> {
    debug_assert!(ptr::addr_eq(guard, unsafe { MOUNT_TABLE.data() }));
    if path.len() > PATH_MAX {
        // There is no distinction between the errno for NAME_MAX exceeded or PATH_MAX exceeded.
        return Err(Errno::ENAMETOOLONG);
    }

    /// A string slice or an arc of dentcache.
    enum LinkValue<'a> {
        None,
        Slice(&'a [u8]),
        Dent(Arc<DentCache>),
    }
    /// An entry on the symlink stack.
    struct LinkEntry<'a> {
        name: LinkValue<'a>,
        offset: usize,
    }
    impl LinkEntry<'_> {
        fn name(&self) -> &[u8] {
            match &self.name {
                LinkValue::None => panic!(),
                LinkValue::Slice(x) => x,
                LinkValue::Dent(x) => x.readlink().unwrap(),
            }
        }
    }

    // The path stack.
    let mut stack = [const {
        LinkEntry {
            name: LinkValue::None,
            offset: 0,
        }
    }; LINK_MAX];
    let mut depth = 0usize;
    let mut links_passed = 0usize;
    stack[0].name = LinkValue::Slice(path);

    // If the path starts with `/`, set `at` to root.
    if path.len() == 0 {
        return Err(Errno::ENOENT);
    } else if path[0] == b'/' {
        at = root_vnode_unlocked(guard)?
            .mtx
            .lock_shared()?
            .dentcache
            .clone()
            .unwrap();
    }

    loop {
        if stack[depth].offset >= stack[depth].name().len() {
            // Pop empty names.
            if depth > 0 {
                depth -= 1;
                continue;
            } else {
                break;
            }
        } else if stack[depth].name()[stack[depth].offset] == b'/' {
            // Skip forward slashes.
            match &at.type_ {
                DentCacheType::Negative => return Err(Errno::ENOENT),
                DentCacheType::Directory(_) => (),
                _ => return Err(Errno::ENOTDIR),
            }
            stack[depth].offset += 1;
            continue;
        }

        // Find next forward slash in the name.
        let name = stack[depth].name();
        let offset = stack[depth].offset;
        let component_len = name[offset..name.len()]
            .iter()
            .position(|x| *x == b'/')
            .unwrap_or(name.len() - offset);
        if component_len > NAME_MAX {
            return Err(Errno::ENAMETOOLONG);
        }

        // Get next component.
        let next = at.lookup(&name[offset..offset + component_len])?;
        stack[depth].offset += component_len;

        let has_more_path = depth > 0 || stack[depth].offset < stack[depth].name().len();
        match &next.type_ {
            DentCacheType::Negative if has_more_path => return Err(Errno::ENOENT),
            DentCacheType::Symlink(_) => {
                if follow_last_symlink || has_more_path {
                    // Follow the symlink.
                    if links_passed >= LINK_MAX {
                        return Err(Errno::EMLINK);
                    }
                    links_passed += 1;
                    depth += 1;
                    stack[depth] = LinkEntry {
                        name: LinkValue::Dent(next.clone()),
                        offset: 0,
                    };
                    if next.readlink()?.len() == 0 {
                        return Err(Errno::ENOENT);
                    } else if next.readlink()?[0] == b'/' {
                        at = root_vnode_unlocked(guard)?
                            .mtx
                            .lock_shared()?
                            .dentcache
                            .clone()
                            .unwrap();
                    }
                }
            }
            _ => at = next,
        }
    }

    Ok(at)
}

/// Walk down the filesystem to a certain path.
fn walk(at: Arc<DentCache>, path: &[u8], follow_last_symlink: bool) -> EResult<Arc<DentCache>> {
    walk_unlocked(at, path, follow_last_symlink, &*MOUNT_TABLE.lock_shared()?)
}

/// Helper function for [`oflags::CREATE`] logic in [`open`].
fn o_creat_helper(to_create: Arc<DentCache>, exclusive: bool) -> EResult<Arc<VNode>> {
    let uses_inodes = to_create.vfs.ops.lock_shared()?.uses_inodes();
    let dir_cache = to_create.parent.clone().unwrap();
    let mut guard = dir_cache.type_.as_dir().unwrap().lock()?;

    // Check whether the file was created in the mean time.
    if let Some(weak) = guard.children.get(&*to_create.dirent.name) {
        if let Some(arc) = weak.upgrade() {
            match &arc.type_ {
                DentCacheType::Negative => (),
                DentCacheType::Directory(_) => return Err(Errno::EISDIR),
                DentCacheType::Symlink(_) => {
                    logkf!(
                        LogLevel::Warning,
                        "TODO: Determine sematics for symlink created in oflags::CREATE race condition"
                    );
                    return Err(Errno::EAGAIN);
                }
                DentCacheType::File => {
                    if exclusive {
                        return Err(Errno::EEXIST);
                    } else {
                        return arc.open_vnode();
                    }
                }
            }
        }
    }

    // A new regular file is to be created.
    let dir_vnode = dir_cache.open_vnode()?;
    let mut dir_guard = dir_vnode.mtx.lock()?;
    if dir_guard.flags & vnflags::REMOVED != 0 {
        return Err(Errno::ENOENT);
    }
    let (dirent, new_ops) =
        dir_guard
            .ops
            .make_file(&dir_vnode, &to_create.dirent.name, MakeFileSpec::Regular)?;
    let ino = if uses_inodes {
        new_ops.get_inode()
    } else {
        dir_vnode.vfs.next_fake_ino.fetch_add(1, Ordering::Relaxed)
    };

    // Create updated dirent cache entry.
    let dentcache = Arc::try_new(DentCache {
        type_: DentCacheType::File,
        vfs: dir_vnode.vfs.clone(),
        parent: Some(dir_guard.dentcache.clone().unwrap()),
        vnode: Mutex::new(None),
        dirent,
    })?;

    // Create new VNode.
    let new_vnode = Arc::try_new(VNode {
        mtx: Mutex::new(VNodeMtxInner {
            ops: new_ops,
            flags: 0,
            dentcache: (!uses_inodes).then(|| dentcache.clone()),
        }),
        ino,
        vfs: dir_vnode.vfs.clone(),
        type_: NodeType::Regular,
        fifo: None,
        pagecache: Some(PageCache::new(dir_vnode.vfs.block_size_exp, 0)),
        denywrite: AtomicU32::new(0),
    })?;
    *dentcache.vnode.unintr_lock() = Some(Arc::downgrade(&new_vnode));

    // Successfully created new VNode.
    if uses_inodes {
        dir_vnode
            .vfs
            .vnodes
            .unintr_lock()
            .insert(ino, Arc::downgrade(&new_vnode));
    }

    // Replace the dirent cache entry.
    guard.children.insert(
        dentcache.dirent.name.clone().into(),
        Arc::downgrade(&dentcache),
    );

    Ok(new_vnode)
}

/// Open a file.
pub fn open(at: Option<&dyn File>, path: &[u8], mut oflags: OFlags) -> EResult<Arc<dyn File>> {
    // Validate oflags.
    if oflags & oflags::DIR_ONLY != 0
        && oflags
            & (oflags::CREATE
                | oflags::EXCLUSIVE
                | oflags::FILE_ONLY
                | oflags::WRITE_ONLY
                | oflags::APPEND
                | oflags::TRUNCATE)
            != 0
    {
        // A flag incompatible with DIR_ONLY was passed.
        return Err(Errno::EINVAL);
    } else if oflags & oflags::APPEND != 0 && oflags & oflags::WRITE_ONLY == 0 {
        // Append requires write.
        return Err(Errno::EINVAL);
    } else if oflags & oflags::READ_WRITE == 0 {
        // Neither read nor write requested; assume the user would like to read.
        oflags |= oflags::READ_ONLY;
    } else if oflags & oflags::EXCLUSIVE != 0 && oflags & oflags::CREATE == 0 {
        // Exclusive without create can never succeed.
        return Err(Errno::EINVAL);
    }

    // Find target file.
    let at = at_vnode(at)?;
    let cache = walk(
        at.mtx
            .lock_shared()?
            .dentcache
            .clone()
            .ok_or(Errno::ENOTDIR)?,
        path,
        oflags & oflags::NOFOLLOW == 0,
    )?;

    // Open the target VNode.
    let vnode = match &cache.type_ {
        DentCacheType::Negative => {
            if oflags & oflags::CREATE == 0 {
                return Err(Errno::ENOENT);
            } else if cache.vfs.is_read_only() {
                return Err(Errno::EROFS);
            }
            o_creat_helper(cache.clone(), oflags & oflags::EXCLUSIVE != 0)?
        }
        DentCacheType::Directory(_) => {
            if oflags & oflags::EXCLUSIVE != 0 {
                return Err(Errno::EEXIST);
            } else if oflags & oflags::FILE_ONLY != 0 {
                return Err(Errno::EISDIR);
            } else {
                cache.open_vnode()?
            }
        }
        _ => {
            if oflags & oflags::DIR_ONLY != 0 {
                return Err(Errno::ENOTDIR);
            } else {
                cache.open_vnode()?
            }
        }
    };

    match vnode.type_ {
        NodeType::Fifo => {
            // FIFO file ops.
            Ok(Box::<dyn File>::from(Box::try_new(Fifo::new(
                Some(vnode.clone()),
                oflags,
                vnode.fifo.clone().unwrap(),
            )?)?)
            .into())
        }
        NodeType::CharDev => {
            // Character device file ops.
            Ok(
                Box::<dyn File>::from(Box::try_new(CharDevFile::new(vnode.clone(), oflags))?)
                    .into(),
            )
        }
        NodeType::BlockDev => {
            // Block device file ops.
            Ok(
                Box::<dyn File>::from(Box::try_new(BlockDevFile::new(vnode.clone(), oflags))?)
                    .into(),
            )
        }
        NodeType::UnixSocket => {
            logkf!(LogLevel::Warning, "TODO: UNIX domain socket file ops");
            return Err(Errno::ENOSYS);
        }
        _ => {
            // Regular file ops.
            if vnode.vfs.flags.load(Ordering::Relaxed) & mflags::READ_ONLY != 0
                && oflags & oflags::WRITE_ONLY != 0
            {
                return Err(Errno::EROFS);
            }
            Ok(Box::<dyn File>::from(Box::try_new(VfsFile {
                vnode,
                flags: Mutex::new(FlagsAndOffset {
                    offset: 0,
                    flags: oflags,
                }),
            })?)
            .into())
        }
    }
}

/// Create a new name for a file.
pub fn link(
    old_at: Option<&dyn File>,
    old_path: &[u8],
    new_at: Option<&dyn File>,
    new_path: &[u8],
    flags: LinkFlags,
) -> EResult<()> {
    // Find source and destination.
    let old_at = at_vnode(old_at)?;
    let new_at = at_vnode(new_at)?;
    let old = walk(
        old_at
            .mtx
            .lock_shared()?
            .dentcache
            .clone()
            .ok_or(Errno::ENOTDIR)?,
        old_path,
        flags & linkflags::FOLLOW_LINKS != 0,
    )?;
    let new = walk(
        new_at
            .mtx
            .lock_shared()?
            .dentcache
            .clone()
            .ok_or(Errno::ENOTDIR)?,
        new_path,
        flags & linkflags::FOLLOW_LINKS != 0,
    )?;

    if old.type_.as_dir().is_some() {
        return Err(Errno::EISDIR);
    }

    // Get parent dirent caches.
    let old_dir_cache = old.parent.clone().ok_or(Errno::EISDIR)?;
    let new_dir_cache = old.parent.clone().ok_or(Errno::EISDIR)?;
    if !Arc::ptr_eq(&old_dir_cache.vfs, &new_dir_cache.vfs) {
        return Err(Errno::EXDEV);
    } else if new_dir_cache.vfs.is_read_only() {
        return Err(Errno::EROFS);
    }

    let old_guard = old_dir_cache.type_.as_dir().unwrap().lock()?;
    let new_guard = if !Arc::ptr_eq(&old_dir_cache, &new_dir_cache) {
        Some(new_dir_cache.type_.as_dir().unwrap().lock()?)
    } else {
        None
    };

    // Perform link operation on VFS.
    let dir_vnode = new_dir_cache.open_vnode()?;
    let mut dir_guard = dir_vnode.mtx.lock()?;
    if dir_guard.flags & vnflags::REMOVED != 0 {
        return Err(Errno::ENOENT);
    }
    dir_guard
        .ops
        .link(&dir_vnode, &new.dirent.name, &*old.open_vnode()?)?;

    // Invalidate dentcache for new.
    new_guard
        .unwrap_or(old_guard)
        .children
        .remove(&*new.dirent.name);

    Ok(())
}

/// Remove a file or directory.
/// Uses POSIX `rmdir` semantics iff `is_rmdir`, otherwise POSIX unlink semantics.
pub fn unlink(at: Option<&dyn File>, path: &[u8], is_rmdir: bool) -> EResult<()> {
    // Find target file.
    let at = at_vnode(at)?;
    let to_remove = walk(
        at.mtx
            .lock_shared()?
            .dentcache
            .clone()
            .ok_or(Errno::ENOTDIR)?,
        path,
        false,
    )?;

    // Get parent dirent cache.
    let dir_cache = to_remove.parent.clone().ok_or(
        // If no parent, this is the root directory of a VFS.
        if is_rmdir {
            Errno::ENOTEMPTY
        } else {
            Errno::EISDIR
        },
    )?;
    if dir_cache.vfs.is_read_only() {
        return Err(Errno::EROFS);
    }
    let mut guard = dir_cache.type_.as_dir().unwrap().lock()?;

    // Get the VNode entry being unlinked.
    let to_remove_vnode = to_remove.vnode.lock_shared()?;
    let unlinked_vnode: Option<_> = try { to_remove_vnode.clone()?.upgrade()? };

    // If the target is a dir, lock it so it cannot be concurrently modified.
    let _target_guard = to_remove.type_.as_dir().map(Mutex::lock);

    // Unlink the node.
    let dir_vnode = dir_cache.open_vnode()?;
    let mut dir_guard = dir_vnode.mtx.lock()?;
    if dir_guard.flags & vnflags::REMOVED != 0 {
        return Err(Errno::ENOENT);
    }
    dir_guard.ops.unlink(
        &dir_vnode,
        &to_remove.dirent.name,
        if is_rmdir {
            UnlinkMode::DirOnly
        } else {
            UnlinkMode::FileOnly
        },
        unlinked_vnode,
    )?;

    // Delete the dirent cache entry.
    guard.children.remove(&*to_remove.dirent.name);

    Ok(())
}

/// Create a new file or directory.
pub fn make_file(at: Option<&dyn File>, path: &[u8], spec: MakeFileSpec) -> EResult<()> {
    // Find target file.
    let at = at_vnode(at)?;
    let to_create = walk(
        at.mtx
            .lock_shared()?
            .dentcache
            .clone()
            .ok_or(Errno::ENOTDIR)?,
        path,
        false,
    )?;
    let uses_inodes = to_create.vfs.ops.lock_shared()?.uses_inodes();

    let dir_cache = to_create.parent.clone().ok_or(Errno::EEXIST)?;
    let mut guard = dir_cache.type_.as_dir().unwrap().lock()?;

    // Check whether the file already exists.
    if let Some(weak) = guard.children.get(&*to_create.dirent.name) {
        if let Some(arc) = weak.upgrade() {
            match &arc.type_ {
                DentCacheType::Negative => (),
                _ => return Err(Errno::EEXIST),
            }
        }
    }

    let dir_vnode = dir_cache.open_vnode()?;
    let mut dir_guard = dir_vnode.mtx.lock()?;
    if dir_guard.flags & vnflags::REMOVED != 0 {
        return Err(Errno::ENOENT);
    }

    if dir_vnode.vfs.is_read_only() {
        return Err(Errno::EROFS);
    }

    // A new file is to be created.
    let type_ = spec.node_type();
    let (dirent, new_ops) =
        dir_guard
            .ops
            .make_file(&dir_vnode, &to_create.dirent.name, spec.clone())?;
    let inode = if uses_inodes {
        new_ops.get_inode()
    } else {
        dir_vnode.vfs.next_fake_ino.fetch_add(1, Ordering::Relaxed)
    };

    // Create updated dirent cache entry.
    let dentcache = Arc::try_new(DentCache {
        type_: match &spec {
            MakeFileSpec::Directory => DentCacheType::Directory(Mutex::new(DentCacheDir {
                children: BTreeMap::new(),
                mounted: None,
            })),
            MakeFileSpec::Symlink(_) => DentCacheType::Symlink(dirent.name.clone().into()),
            _ => DentCacheType::File,
        },
        vfs: dir_vnode.vfs.clone(),
        parent: Some(dir_guard.dentcache.clone().unwrap()),
        vnode: Mutex::new(None),
        dirent,
    })?;

    // Create new VNode.
    let fifo = (type_ == NodeType::Fifo).then(|| FifoShared::new());
    let pagecache =
        (type_ == NodeType::Regular).then(|| PageCache::new(dir_vnode.vfs.block_size_exp, 0));
    let new_vnode = Arc::try_new(VNode {
        mtx: Mutex::new(VNodeMtxInner {
            ops: new_ops,
            flags: 0,
            dentcache: None,
        }),
        ino: inode,
        vfs: dir_vnode.vfs.clone(),
        type_,
        fifo,
        pagecache,
        denywrite: AtomicU32::new(0),
    })?;
    *dentcache.vnode.unintr_lock() = Some(Arc::downgrade(&new_vnode));

    // Successfully created new VNode.
    dir_vnode
        .vfs
        .vnodes
        .unintr_lock()
        .insert(inode, Arc::downgrade(&new_vnode));

    // Replace the dirent cache entry.
    guard.children.insert(
        dentcache.dirent.name.clone().into(),
        Arc::downgrade(&dentcache),
    );

    Ok(())
}

/// Rename a file within the same filesystem.
/// TODO: Default POSIX semantics actually delete the target, this doesn't.
pub fn rename(
    old_at: Option<&dyn File>,
    old_path: &[u8],
    new_at: Option<&dyn File>,
    new_path: &[u8],
    flags: LinkFlags,
) -> EResult<()> {
    loop {
        let res = rename_impl(old_at, old_path, new_at, new_path, flags);
        if res != Err(Errno::ETIMEDOUT) {
            return res;
        }
    }
}

/// Assert a rename would not move a directory inside itself.
fn rename_check_loops(old: &DentCache, mut new: &DentCache) -> EResult<()> {
    loop {
        if let Some(parent) = &new.parent {
            new = parent.as_ref();
        } else {
            // Reached root without seeing `old`.
            return Ok(());
        }
        if ptr::addr_eq(old, new) {
            // Reached `old` from `new`; renaming would cause a loop here.
            return Err(Errno::EINVAL);
        }
    }
}

/// Rename a file within the same filesystem.
/// May time out on mutexes (to avoid deadlocks).
fn rename_impl(
    old_at: Option<&dyn File>,
    old_path: &[u8],
    new_at: Option<&dyn File>,
    new_path: &[u8],
    flags: u32,
) -> EResult<()> {
    // Find source and destination.
    let old_at = at_vnode(old_at)?;
    let new_at = at_vnode(new_at)?;
    let old = walk(
        old_at
            .mtx
            .lock_shared()?
            .dentcache
            .clone()
            .ok_or(Errno::ENOTDIR)?,
        old_path,
        false,
    )?;
    if old.type_.is_negative() {
        return Err(Errno::ENOENT);
    }
    let new = walk(
        new_at
            .mtx
            .lock_shared()?
            .dentcache
            .clone()
            .ok_or(Errno::ENOTDIR)?,
        new_path,
        false,
    )?;

    // Get parent dirent caches.
    let old_dir_cache = old.parent.clone().ok_or(Errno::EBUSY)?;
    let new_dir_cache = new.parent.clone().ok_or(Errno::EBUSY)?;
    let old_dir_vnode = old_dir_cache.open_vnode()?;

    let mut old_guard = old_dir_cache.type_.as_dir().unwrap().lock()?;
    let (new_guard, dirent) = if Arc::ptr_eq(&old_dir_cache, &new_dir_cache) {
        // Rename within directory.
        let mut old_dir_guard = old_dir_vnode.mtx.lock()?;

        // Check for race condition where another thread might outdate the dentcache entry for `old`.
        old_guard.check_for_entry(&old)?;

        if old_dir_guard.flags & vnflags::REMOVED != 0 {
            return Err(Errno::ENOENT);
        }
        if old_dir_cache.vfs.is_read_only() {
            return Err(Errno::EROFS);
        }

        // Remove existing file.
        if !new.type_.is_negative() {
            if flags & renameflags::NOREPLACE != 0 {
                return Err(Errno::EEXIST);
            }
            let new_vnode = new.vnode.lock_shared()?;
            let unlinked_vnode: Option<_> = try { new_vnode.clone()?.upgrade()? };
            old_dir_guard.ops.unlink(
                &old_dir_vnode,
                &new.dirent.name,
                UnlinkMode::Any,
                unlinked_vnode,
            )?;
        } else {
            old_guard.check_for_entry(&new)?
        }

        let dirent =
            old_dir_guard
                .ops
                .rename(&old_dir_vnode, &old.dirent.name, &new.dirent.name)?;
        (None, dirent)
    } else {
        // Rename across directories.
        let new_guard = new_dir_cache.type_.as_dir().unwrap().timed_lock(10000)?;
        let new_dir_vnode = new_dir_cache.open_vnode()?;
        let mut old_dir_guard = old_dir_vnode.mtx.lock()?;
        let mut new_dir_guard = new_dir_vnode.mtx.timed_lock(10000)?;

        // Check for race condition where another thread might outdate the dentcache entry for `old`.
        old_guard.check_for_entry(&old)?;

        if old_dir_guard.flags & vnflags::REMOVED != 0 {
            return Err(Errno::ENOENT);
        }
        if new_dir_guard.flags & vnflags::REMOVED != 0 {
            return Err(Errno::ENOENT);
        }
        if !Arc::ptr_eq(&old_dir_cache.vfs, &new_dir_cache.vfs) {
            return Err(Errno::EXDEV);
        }
        if old_dir_cache.vfs.is_read_only() {
            return Err(Errno::EROFS);
        }
        rename_check_loops(&old_dir_cache, &new_dir_cache)?;

        // Remove existing file.
        if !new.type_.is_negative() {
            if flags & renameflags::NOREPLACE != 0 {
                return Err(Errno::EEXIST);
            }
            let new_vnode = new.vnode.lock_shared()?;
            let unlinked_vnode: Option<_> = try { new_vnode.clone()?.upgrade()? };
            old_dir_guard.ops.unlink(
                &old_dir_vnode,
                &new.dirent.name,
                UnlinkMode::Any,
                unlinked_vnode,
            )?;
        } else {
            new_guard.check_for_entry(&new)?
        }

        let dirent = old_dir_cache.vfs.ops.lock_shared()?.rename(
            &old_dir_cache.vfs,
            &old_dir_vnode,
            &old.dirent.name,
            &mut *old_dir_guard,
            &new_dir_vnode,
            &new.dirent.name,
            &mut *new_dir_guard,
        )?;
        (Some(new_guard), dirent)
    };

    // Remove old cache entry.
    let old_dentcache: Option<_> = try { old_guard.children.remove(&*old.dirent.name)?.upgrade()? };

    // Try to update entry.
    if let Some(old_dentcache) = old_dentcache {
        let type_ = match &old_dentcache.type_ {
            DentCacheType::Negative => unreachable!(),
            DentCacheType::Directory(mutex) => {
                DentCacheType::Directory(Mutex::new(mutex.unintr_lock_shared().clone()))
            }
            DentCacheType::Symlink(value) => DentCacheType::Symlink(value.clone()),
            DentCacheType::File => DentCacheType::File,
        };

        let moved_vnode: Option<Arc<VNode>> = try {
            old_dentcache
                .vnode
                .unintr_lock_shared()
                .clone()?
                .upgrade()?
        };

        // Create new dirent cache entry.
        let dentcache = Arc::new(DentCache {
            type_,
            vfs: new_dir_cache.vfs.clone(),
            parent: Some(new_dir_cache.clone()),
            vnode: Mutex::new(moved_vnode.clone().map(|x| Arc::downgrade(&x))),
            dirent: dirent.clone(),
        });

        // Update that of the VNode if it exists and has a dentcache reference.
        if let Some(moved_vnode) = &moved_vnode {
            let mut vnode_guard = moved_vnode.mtx.unintr_lock();
            if vnode_guard.dentcache.is_some() {
                vnode_guard.dentcache = Some(dentcache.clone());
            }
        }

        // Insert updated entry.
        new_guard
            .unwrap_or(old_guard)
            .children
            .insert(dirent.name.clone(), Arc::downgrade(&dentcache));
    } else {
        // Possibly remove stale entry.
        new_guard.unwrap_or(old_guard).children.remove(&dirent.name);
    }

    Ok(())
}

/// Get the real path from some canonical path.
pub fn realpath(at: Option<&dyn File>, path: &[u8], follow_last_symlink: bool) -> EResult<Vec<u8>> {
    let at = at_vnode(at)?;
    let cache = walk(
        at.mtx
            .lock_shared()?
            .dentcache
            .clone()
            .ok_or(Errno::ENOTDIR)?,
        path,
        follow_last_symlink,
    )?;
    cache.realpath()
}

/// Convert into an absolute path.
/// May return a path longer than [`PATH_MAX`].
pub fn abspath(old_cwd: &[u8], chdir_path: &[u8]) -> EResult<Vec<u8>> {
    if old_cwd.len() > PATH_MAX || chdir_path.len() > PATH_MAX {
        return Err(Errno::ENAMETOOLONG);
    }

    let mut cur;
    if chdir_path.first().cloned() == Some(b'/') {
        cur = Vec::from(chdir_path);
    } else {
        cur = Vec::from(old_cwd);
        cur.extend_from_slice(chdir_path);
    }

    let mut i = 0;
    while i < cur.len() {
        if cur[i..].starts_with(b"./") {
            cur.drain(i..i + 2);
        } else if cur[i..].starts_with(b"../") {
            let start = cur[..i]
                .iter()
                .enumerate()
                .rfind(|x| *x.1 == b'/')
                .map(|x| x.0 + 1)
                .unwrap_or(0);
            cur.drain(start..i + 1);
        } else {
            i += 1;
        }
    }

    Ok(cur)
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
    mflags: MFlags,
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
        flags: AtomicU32::new(vfs_ops.read_only() as u32 * mflags::READ_ONLY),
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
            type_: NodeType::Directory,
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
        type_: NodeType::Directory,
        fifo: None,
        pagecache: None,
        denywrite: AtomicU32::new(0),
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
    mflags: MFlags,
) -> EResult<()> {
    // Determine filesystem type.
    let drivers = FSDRIVERS.lock_shared()?;
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
    let at = at_vnode_unlocked(at, &mounts)?;
    let cache = walk_unlocked(
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
    unsafe extern "C" {
        fn device_devtmpfs_mounted(devtmpfs_root: file_t) -> errno_t;
    }
    if let EResult::Err(x) = try {
        let vfs_root_dir = open(orig_at, path, oflags::DIR_ONLY | oflags::READ_ONLY)?;
        Errno::check(unsafe { device_devtmpfs_mounted(ref_as_file(&*vfs_root_dir)) })?;
    } {
        logkf!(LogLevel::Warning, "Failed to populate devtmpfs: {}", x);
    }

    Ok(())
}

/// Unmount an existing filesystem by mountpoint or device.
pub fn umount(at: Option<&dyn File>, path: &[u8], flags: MFlags) -> EResult<()> {
    // Open the media / VFS root VNode.
    let target = open(at, path, flags & oflags::NOFOLLOW)?
        .get_vnode()
        .unwrap();

    // Now, lock the mount table; this will inhibit any new VNodes from being opened.
    let mut mount_table = MOUNT_TABLE.lock()?;
    // Get the target VFS from this VNode.
    let mut vfs = target.follow_mounts().is_vfs_root();
    if vfs.is_none() {
        let ops = &target.mtx.lock_shared()?.ops;
        vfs = try {
            let device = ops.get_device(&target)?.as_block()?;
            let offset = ops.get_part_offset(&target);
            let media_key = MediaKey { device, offset };
            mount_table.fs_by_media.get(&media_key).cloned()?
        };
    }
    let vfs = vfs.ok_or(Errno::ENOENT)?;

    // Assert that no files are open; only the root dir VNode should be present with refcount 2.
    if flags & mflags::DETACH == 0 {
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

/// Create an unnamed pipe.
/// The end written to is 0, the end read from is 1.
pub fn pipe(oflags: OFlags) -> EResult<(Arc<dyn File>, Arc<dyn File>)> {
    // TODO: OOM handling.
    let shared = FifoShared::new();
    shared.open(true, true, true)?;
    let write_end = Arc::new(Fifo::new(
        None,
        (oflags & oflags::NONBLOCK) | oflags::WRITE_ONLY,
        shared.clone(),
    )?);
    let read_end = Arc::new(Fifo::new(
        None,
        (oflags & oflags::NONBLOCK) | oflags::READ_ONLY,
        shared,
    )?);
    Ok((write_end, read_end))
}
