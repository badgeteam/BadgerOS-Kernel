// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use static_assertions::assert_eq_size;

use crate::{bindings::error::Errno, filesystem::NodeType};

/// Calls conversion function `$func` on integers in `$type`.
#[rustfmt::skip]
macro_rules! int_conv {
    // Implementations on all primitive integer types.
    ($func: ident,  $name: expr,  u8  ) => { $name = u8  ::$func($name); };
    ($func: ident,  $name: expr,  i8  ) => { $name = i8  ::$func($name); };
    ($func: ident,  $name: expr,  u16 ) => { $name = u16 ::$func($name); };
    ($func: ident,  $name: expr,  i16 ) => { $name = i16 ::$func($name); };
    ($func: ident,  $name: expr,  u32 ) => { $name = u32 ::$func($name); };
    ($func: ident,  $name: expr,  i32 ) => { $name = i32 ::$func($name); };
    ($func: ident,  $name: expr,  u64 ) => { $name = u64 ::$func($name); };
    ($func: ident,  $name: expr,  i64 ) => { $name = i64 ::$func($name); };
    ($func: ident,  $name: expr,  u128) => { $name = u128::$func($name); };
    ($func: ident,  $name: expr,  i128) => { $name = i128::$func($name); };
    // Implementation on arrays.
    ($func: ident,  $name: expr,  [$type: tt; $count: expr]) => {
        for __i in 0..$count {
            int_conv!{ $func, $name[__i], $type }
        }
    };
}

/// Defines an on-disk struct with some helper functions.
macro_rules! struct_def {
    (
        // Parse struct definition.
        $(#[doc = $structdoc: expr])*
        struct $struct: ident {
            $(
                $(#[doc = $namedoc: expr])*
                $name: ident : $type: tt
            ),*
            $(,)?
        }
    ) => {
        // Define base struct.
        #[derive(Clone, Copy)]
        #[repr(C)]
        $(#[doc = $structdoc])*
        pub struct $struct {
            $(
                $(#[doc = $namedoc])*
                pub $name: $type,
            )*
        }
        // Define conversion from bytes.
        impl ::core::convert::From<[u8; ::core::mem::size_of::<$struct>()]> for $struct {
            fn from(value: [u8; ::core::mem::size_of::<$struct>()]) -> Self {
                let mut tmp: Self = unsafe { ::core::mem::transmute(value) };
                $(int_conv!{from_le, tmp.$name, $type})*
                tmp
            }
        }
        impl ::core::convert::From<::alloc::boxed::Box<[u8; ::core::mem::size_of::<$struct>()]>> for ::alloc::boxed::Box<$struct> {
            fn from(value: ::alloc::boxed::Box<[u8; ::core::mem::size_of::<$struct>()]>) -> Self {
                let mut tmp: Self = unsafe { ::core::mem::transmute(value) };
                $(int_conv!{from_le, tmp.$name, $type})*
                tmp
            }
        }
        // Define conversion to bytes.
        impl ::core::convert::Into<[u8; ::core::mem::size_of::<$struct>()]> for $struct {
            fn into(mut self) -> [u8; ::core::mem::size_of::<$struct>()] {
                $(int_conv!{to_le, self.$name, $type})*
                unsafe { ::core::mem::transmute(self) }
            }
        }
        impl ::core::convert::Into<::alloc::boxed::Box<[u8; ::core::mem::size_of::<$struct>()]>> for ::alloc::boxed::Box<$struct> {
            fn into(mut self) -> ::alloc::boxed::Box<[u8; ::core::mem::size_of::<$struct>()]> {
                $(int_conv!{to_le, self.$name, $type})*
                unsafe { ::core::mem::transmute(self) }
            }
        }
        // Define default to be zeroed.
        impl ::core::default::Default for $struct {
            fn default() -> Self {
                unsafe { ::core::mem::zeroed() }
            }
        }
    };
}

/// Bad blocks inode number.
pub const BADBLOCKS_INO: u32 = 1;
/// Root directory inode number.
pub const ROOT_INO: u32 = 2;

/// Identifies a filesystem as EXT2; superblock field [`Superblock::magic`].
pub const MAGIC: u16 = 0xef53;

#[rustfmt::skip]
pub mod feat {
    pub mod compat {
        /// Block pre-allocation for new directories
        pub const DIR_PREALLOC:  u32 = 0x0001;
        pub const IMAGIC_INODES: u32 = 0x0002;
        /// An Ext3 journal exists
        pub const HAS_JOURNAL:   u32 = 0x0004;
        /// Extended inode attributes are present
        pub const EXT_ATTR:      u32 = 0x0008;
        /// Non-standard inode size used
        pub const RESIZE_INO:    u32 = 0x0010;
        /// Directory indexing (HTree)
        pub const DIR_INDEX:     u32 = 0x0020;
    }
    
    pub mod incompat {
        /// Disk/File compression is used
        pub const COMPRESSION: u32 = 0x0001;
        pub const FILETYPE:    u32 = 0x0002;
        pub const RECOVER:     u32 = 0x0004;
        pub const JOURNAL_DEV: u32 = 0x0008;
        pub const META_BG:     u32 = 0x0010;
    }
    
    pub mod compat_ro {
        /// Sparse Superblock
        pub const SPARSE_SUPER: u32 = 0x0001;
        /// Large file support, 64-bit file size
        pub const LARGE_FILE:   u32 = 0x0002;
        /// Binary tree sorted directory files
        pub const BTREE_DIR:    u32 = 0x0004;
    }
}

struct_def! {
    /// EXT2 superblock; filesystem metadata.
    struct Superblock {
        /* ==== Generic superblock information ==== */
        inode_count:       u32,
        block_count:       u32,
        resvd_block_count: u32,
        free_block_count:  u32,
        free_inode_count:  u32,
        first_data_block:  u32,
        block_size_exp:    u32,
        frag_size_exp:     u32,
        blocks_per_group:  u32,
        frags_per_group:   u32,
        inodes_per_group:  u32,
        mtime:             u32,
        wtime:             u32,
        mnt_count:         u16,
        max_mnt_count:     u16,
        magic:             u16,
        state:             u16,
        errors:            u16,
        minor_rev_level:   u16,
        lastcheck:         u32,
        checkinterval:     u32,
        creator_os:        u32,
        rev_level:         u32,
        def_resuid:        u16,
        def_resgid:        u16,

        /* ==== EXT2_DYNAMIC_REV Specific ==== */
        first_ino:         u32,
        inode_size:        u16,
        block_group_nr:    u16,
        feature_compat:    u32,
        feature_incompat:  u32,
        feature_ro_compat: u32,
        uuid:              [u32; 4],
        volume_name:       [u8; 16],
        last_mounted:      [u8; 64],
        algo_bitmap:       u32,

        /* ==== Performance hints ==== */
        prealloc_blocks:     u8,
        prealloc_dir_blocks: u8,
        _padding0:           [u8; 2],

        /* ==== Journalling support ==== */
        journal_uuid: [u32; 4],
        journal_inum: u32,
        journal_dev:  u32,
        last_orphan:  u32,

        /* ==== Directory indexing support ==== */
        hash_seed:    [u32; 4],
        hash_version: u8,
        _padding1:    [u8; 3],

        /* ==== Other options ==== */
        default_mount_options: u32,
        first_meta_blockgroup: u32,
    }
}
assert_eq_size!(Superblock, [u8; 264]);

struct_def! {
    /// Block group descriptor table.
    struct BlockGroupDesc {
        /// Index of the first block in this group.
        /// The block bitmap also starts here.
        block_bitmap:     u32,
        /// Index of the first block of the inode bitmap in this group.
        inode_bitmap:     u32,
        /// Index of the first block of the inode table in this group.
        inode_table:      u32,
        /// Number of free blocks in this group.
        free_block_count: u16,
        /// Number of free inodes in this group.
        free_inode_count: u16,
        /// Number of inodes that are directories in this group.
        used_dirs_count:  u16,
        _resvd0:          u16,
        _resvd1:          [u8; 12],
    }
}
assert_eq_size!(BlockGroupDesc, [u8; 32]);

/// Ext2 node types as seen in the mode fields.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// UNIX socket.
    UnixSocket = 0xC000,
    /// Symlink.
    Symlink = 0xA000,
    /// Regular file.
    Regular = 0x8000,
    /// Block device.
    BlockDev = 0x6000,
    /// Directory.
    Directory = 0x4000,
    /// Character device.
    CharDev = 0x2000,
    /// FIFO / named pipe.
    Fifo = 0x1000,
}

impl Into<NodeType> for Mode {
    fn into(self) -> NodeType {
        match self {
            Mode::UnixSocket => NodeType::UnixSocket,
            Mode::Symlink => NodeType::Symlink,
            Mode::Regular => NodeType::Regular,
            Mode::BlockDev => NodeType::BlockDev,
            Mode::Directory => NodeType::Directory,
            Mode::CharDev => NodeType::CharDev,
            Mode::Fifo => NodeType::Fifo,
        }
    }
}

impl TryFrom<NodeType> for Mode {
    type Error = Errno;

    fn try_from(value: NodeType) -> Result<Self, Errno> {
        match value {
            NodeType::UnixSocket => Ok(Mode::UnixSocket),
            NodeType::Symlink => Ok(Mode::Symlink),
            NodeType::Regular => Ok(Mode::Regular),
            NodeType::BlockDev => Ok(Mode::BlockDev),
            NodeType::Directory => Ok(Mode::Directory),
            NodeType::CharDev => Ok(Mode::CharDev),
            NodeType::Fifo => Ok(Mode::Fifo),
            _ => Err(Errno::EINVAL),
        }
    }
}

impl TryFrom<u16> for Mode {
    type Error = Errno;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value & 0xf000 {
            0xC000 => Ok(Self::UnixSocket),
            0xA000 => Ok(Self::Symlink),
            0x8000 => Ok(Self::Regular),
            0x6000 => Ok(Self::BlockDev),
            0x4000 => Ok(Self::Directory),
            0x2000 => Ok(Self::CharDev),
            0x1000 => Ok(Self::Fifo),
            _ => Err(Errno::EIO),
        }
    }
}

struct_def! {
    /// Inode structure.
    struct Inode {
        mode:        u16,
        uid:         u16,
        size:        u32,
        atime:       u32,
        ctime:       u32,
        mtime:       u32,
        dtime:       u32,
        gid:         u16,
        nlink:       u16,
        /// Actual amount of allocated disk space divide ceil by 512.
        realsize:    u32,
        flags:       u32,
        _osd1:       [u8; 4],
        data_blocks: [u32; 15],
        generation:  u32,
        file_acl:    u32,
        /// For regular files, high 32 bits of size.
        dir_acl:     u32,
        _obsolete:   u32,
        _osd2:       [u8; 12],
    }
}
assert_eq_size!(Inode, [u8; 128]);

/// Ext2 node types as seen in dirent file type fields.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum FileType {
    /// Unknown File Type
    Unknown = 0,
    /// Regular File
    Regular = 1,
    /// Directory File
    Directory = 2,
    /// Character Device
    CharDev = 3,
    /// Block Device
    BlockDev = 4,
    /// Buffer File
    Fifo = 5,
    /// Socket File
    UnixSocket = 6,
    /// Symbolic Link
    Symlink = 7,
}

impl TryFrom<u8> for FileType {
    type Error = Errno;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(FileType::Unknown),
            1 => Ok(FileType::Regular),
            2 => Ok(FileType::Directory),
            3 => Ok(FileType::CharDev),
            4 => Ok(FileType::BlockDev),
            5 => Ok(FileType::Fifo),
            6 => Ok(FileType::UnixSocket),
            7 => Ok(FileType::Symlink),
            _ => Err(Errno::EIO),
        }
    }
}

impl Into<NodeType> for FileType {
    fn into(self) -> NodeType {
        match self {
            FileType::UnixSocket => NodeType::UnixSocket,
            FileType::Symlink => NodeType::Symlink,
            FileType::Regular => NodeType::Regular,
            FileType::BlockDev => NodeType::BlockDev,
            FileType::Directory => NodeType::Directory,
            FileType::CharDev => NodeType::CharDev,
            FileType::Fifo => NodeType::Fifo,
            FileType::Unknown => NodeType::Unknown,
        }
    }
}

impl From<NodeType> for FileType {
    fn from(value: NodeType) -> Self {
        match value {
            NodeType::UnixSocket => FileType::UnixSocket,
            NodeType::Symlink => FileType::Symlink,
            NodeType::Regular => FileType::Regular,
            NodeType::BlockDev => FileType::BlockDev,
            NodeType::Directory => FileType::Directory,
            NodeType::CharDev => FileType::CharDev,
            NodeType::Fifo => FileType::Fifo,
            NodeType::Unknown => FileType::Unknown,
        }
    }
}

struct_def! {
    /// Dirent format for linked list directories.
    /// The name is placed after the struct and is `name_len` bytes long.
    struct LinkedDent {
        ino: u32,
        record_len: u16,
        name_len: u8,
        /// On Ext2 rev. 0, upper 16 bits of `name_len`.
        file_type: u8,
    }
}
assert_eq_size!(LinkedDent, [u8; 8]);

/// Ext2 indexed dirent hash types.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DentHashVer {
    Legacy,
    HalfMd4,
    Tea,
}

struct_def! {
    /// Indexed directory root structure.
    struct IndexedDentRoot {
        /// Reserved; should be 0.
        _resvd0: u32,
        /// What hashing algorithm is used.
        hash_ver: u8,
        /// How large this info struct is.
        info_len: u8,
        /// How many levels of indirect indexing are used (0-1).
        indirect_levels: u8,
        /// Reserved; should be 0.
        _resvd1: u8,
    }
}
assert_eq_size!(IndexedDentRoot, [u8; 8]);

struct_def! {
    /// Indexed directory limit structure.
    struct IndexedDentLimit {
        /// Maximum number of indexed dirents that will fit.
        limit: u16,
        /// Actual number of indexed dirents.
        count: u16,
    }
}
assert_eq_size!(IndexedDentLimit, [u8; 4]);

struct_def! {
    /// Indexed directory entry structure.
    struct IndexedDent {
        /// Hash value of this dirent.
        hash: u32,
        /// Directory block offset of the [`LinkedDirent`] pointed to by this entry.
        block: u32,
    }
}
assert_eq_size!(IndexedDent, [u8; 8]);
