// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#[rustfmt::skip]
pub mod attr {
    pub const READ_ONLY: u8 = 0x01;
    pub const HIDDEN:    u8 = 0x02;
    pub const SYSTEM:    u8 = 0x04;
    pub const VOLUME_ID: u8 = 0x08;
    pub const DIRECTORY: u8 = 0x10;
    pub const ARCHIVE:   u8 = 0x20;
    pub const LONG_NAME: u8 = READ_ONLY | HIDDEN | SYSTEM | VOLUME_ID;
}

#[rustfmt::skip]
pub mod attr2 {
    pub const LC_NAME: u8 = 0x08;
    pub const LC_EXT:  u8 = 0x10;
}

#[repr(packed)]
#[derive(Debug, Default, Clone, Copy)]
/// FAT BIOS parameter block.
pub struct Bpb {
    /// Used for the x86 jump to bootloader.
    pub jumpboot: [u8; 3],
    /// OEM name, recommended is "MSWIN4.1".
    pub oem_name: [u8; 8],
    /// Bytes per physical sector, one of 512, 1024, 2048, 4096.
    pub bytes_per_sector: u16,
    /// Sectors per cluster (allocation unit), power of 2 greater greater than 0.
    /// Note: Clusters larger than 32K are poorly supported by others.
    pub sectors_per_cluster: u8,
    /// Reserved sector count starting at first physical sector.
    /// Note: On FAT12 and FAT16, this value should be 1.
    pub reserved_sector_count: u16,
    /// Number of copies of the FAT, should be 2.
    pub fat_count: u8,
    /// Number file of entries in the root directory.
    /// Note: On FAT32, this is value must be 0.
    pub root_entry_count: u16,
    /// 16-bit total sector count.
    /// If the sector count is more than 65535, `sector_count_32` is used.
    pub sector_count_16: u16,
    /// Media type, 0xF0 or 0xF8 - 0xFF.
    /// Note: The first byte of every FAT must equal this value.
    pub media_type: u8,
    /// 16-bit sectors per FAT count.
    pub sectors_per_fat_16: u16,
    /// Sectors per track for floppy disks.
    pub sectors_per_track: u16,
    /// Number of heads for floppy disks.
    pub head_count: u16,
    /// Number of "hidden" sectors (usually reserved by the bootloader).
    pub hidden_sector_count: u32,
    /// 32-bit total sector count.
    pub sector_count_32: u32,
}
static_assertions::assert_eq_size!(Bpb, [u8; 36]);

impl From<[u8; 36]> for Bpb {
    fn from(value: [u8; 36]) -> Self {
        unsafe { core::mem::transmute(value) }
    }
}

#[rustfmt::skip]
impl Bpb {
    /// Converts all integers from little-endian.
    pub const fn from_le(&mut self) {
        self.bytes_per_sector      = u16::from_le(self.bytes_per_sector);
        self.reserved_sector_count = u16::from_le(self.reserved_sector_count);
        self.root_entry_count      = u16::from_le(self.root_entry_count);
        self.sector_count_16       = u16::from_le(self.sector_count_16);
        self.sectors_per_fat_16    = u16::from_le(self.sectors_per_fat_16);
        self.sectors_per_track     = u16::from_le(self.sectors_per_track);
        self.head_count            = u16::from_le(self.head_count);
        self.hidden_sector_count   = u32::from_le(self.hidden_sector_count);
        self.sector_count_32       = u32::from_le(self.sector_count_32);
    }

    /// Converts all integers into little-endian.
    pub const fn to_le(&mut self) {
        self.bytes_per_sector      = u16::to_le(self.bytes_per_sector);
        self.reserved_sector_count = u16::to_le(self.reserved_sector_count);
        self.root_entry_count      = u16::to_le(self.root_entry_count);
        self.sector_count_16       = u16::to_le(self.sector_count_16);
        self.sectors_per_fat_16    = u16::to_le(self.sectors_per_fat_16);
        self.sectors_per_track     = u16::to_le(self.sectors_per_track);
        self.head_count            = u16::to_le(self.head_count);
        self.hidden_sector_count   = u32::to_le(self.hidden_sector_count);
        self.sector_count_32       = u32::to_le(self.sector_count_32);
    }
}

#[repr(packed)]
#[derive(Debug, Default, Clone, Copy)]
/// FAT12/FAT16 filesystem header.
pub struct Header16 {
    /// Drive number for floppy disks.
    pub drive_number: u8,
    /// Reserved; set to 0.
    pub _reserved0: u8,
    /// Extended boot signature; set to 0x29.
    pub boot_signature: u8,
    /// Volume ID.
    pub volume_id: u32,
    /// Volume label, upper-case ASCII padded with 0x20.
    pub volume_label: [u8; 11],
    /// User-facing filesystem type string, upper-case ASCII padded with 0x20.
    pub filesystem_string: [u8; 8],
}
static_assertions::assert_eq_size!(Header16, [u8; 26]);

impl From<[u8; 26]> for Header16 {
    fn from(value: [u8; 26]) -> Self {
        unsafe { core::mem::transmute(value) }
    }
}

#[rustfmt::skip]
impl Header16 {
    /// Converts all integers from little-endian.
    pub const fn from_le(&mut self) {
        self.volume_id = u32::from_le(self.volume_id);
    }

    /// Converts all integers into little-endian.
    pub const fn to_le(&mut self) {
        self.volume_id = u32::to_le(self.volume_id);
    }
}

#[repr(packed)]
#[derive(Debug, Default, Clone, Copy)]
/// FAT32 filesystem header.
pub struct Header32 {
    /// 32-bit sectors per FAT.
    pub sectors_per_fat_32: u32,
    /// Extra filesystem flags.
    pub extra_flags: u16,
    /// Filesystem version; set to 0.
    pub fs_version: u16,
    /// First cluster of the root directory, usually 2.
    pub first_root_cluster: u32,
    /// Sector number of the active filesystem info structure.
    pub fs_info_sector: u16,
    /// Sector number of the backup bootsector, should be 6.
    pub backup_bootsector: u16,
    /// Reserved, set to 0.
    pub _reserved0: [u8; 12],
}
static_assertions::assert_eq_size!(Header32, [u8; 28]);

impl From<[u8; 28]> for Header32 {
    fn from(value: [u8; 28]) -> Self {
        unsafe { core::mem::transmute(value) }
    }
}

#[rustfmt::skip]
impl Header32 {
    /// Converts all integers from little-endian.
    pub const fn from_le(&mut self) {
        self.sectors_per_fat_32 = u32::from_le(self.sectors_per_fat_32);
        self.extra_flags        = u16::from_le(self.extra_flags);
        self.fs_version         = u16::from_le(self.fs_version);
        self.first_root_cluster = u32::from_le(self.first_root_cluster);
        self.fs_info_sector     = u16::from_le(self.fs_info_sector);
        self.backup_bootsector  = u16::from_le(self.backup_bootsector);
    }

    /// Converts all integers into little-endian.
    pub const fn to_le(&mut self) {
        self.sectors_per_fat_32 = u32::to_le(self.sectors_per_fat_32);
        self.extra_flags        = u16::to_le(self.extra_flags);
        self.fs_version         = u16::to_le(self.fs_version);
        self.first_root_cluster = u32::to_le(self.first_root_cluster);
        self.fs_info_sector     = u16::to_le(self.fs_info_sector);
        self.backup_bootsector  = u16::to_le(self.backup_bootsector);
    }
}

/// Pack a FAT date into a [`u16`].
///
/// # Arguments
/// * `year` - Years passed since 1980
/// * `month` - Month number (1-12)
/// * `day` - Day number (1-31)
pub const fn pack_date(year: u8, month: u8, day: u8) -> u16 {
    day as u16 | ((month as u16) << 5) | ((year as u16) << 9)
}

/// Unpack a FAT date from a [`u16`].
///
/// # Returns
/// A tuple containing:
/// * Years passed since 1980
/// * Month number (1-12)
/// * Day number (1-31)
pub const fn unpack_date(raw: u16) -> (u8, u8, u8) {
    ((raw >> 9) as u8, ((raw >> 5) & 15) as u8, (raw & 31) as u8)
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
/// FAT directory entry.
pub struct Dirent {
    /// Short filename in 8.3 format.
    pub name: [u8; 11],
    /// File attributes.
    pub attr: u8,
    /// Additional attributes.
    pub attr2: u8,
    /// Creation time in 0.1s increments.
    pub ctime_tenth: u8,
    /// Creation time in 2s increments.
    pub ctime_2s: u16,
    /// Creation date.
    pub ctime: u16,
    /// Last accessed date.
    pub atime: u16,
    /// High 16 bits of first cluster.
    pub first_cluster_hi: u16,
    /// Modification time in 2s increments.
    pub mtime_2s: u16,
    /// Modification date.
    pub mtime: u16,
    /// Low 16 bits of first cluster.
    pub first_cluster_lo: u16,
    /// File size in bytes.
    pub size: u32,
}
static_assertions::assert_eq_size!(Dirent, [u8; 32]);

#[rustfmt::skip]
impl Dirent {
    /// Converts all integers from little-endian.
    pub const fn from_le(&mut self) {
        self.ctime_2s         = u16::from_le(self.ctime_2s);
        self.ctime            = u16::from_le(self.ctime);
        self.atime            = u16::from_le(self.atime);
        self.first_cluster_hi = u16::from_le(self.first_cluster_hi);
        self.mtime_2s         = u16::from_le(self.mtime_2s);
        self.mtime            = u16::from_le(self.mtime);
        self.first_cluster_lo = u16::from_le(self.first_cluster_lo);
        self.size             = u32::from_le(self.size);
    }

    /// Converts all integers into little-endian.
    pub const fn to_le(&mut self) {
        self.ctime_2s         = u16::to_le(self.ctime_2s);
        self.ctime            = u16::to_le(self.ctime);
        self.atime            = u16::to_le(self.atime);
        self.first_cluster_hi = u16::to_le(self.first_cluster_hi);
        self.mtime_2s         = u16::to_le(self.mtime_2s);
        self.mtime            = u16::to_le(self.mtime);
        self.first_cluster_lo = u16::to_le(self.first_cluster_lo);
        self.size             = u32::to_le(self.size);
    }
}

impl Into<LfnEnt> for Dirent {
    fn into(self) -> LfnEnt {
        unsafe { core::mem::transmute(self) }
    }
}

impl From<[u8; 32]> for Dirent {
    fn from(value: [u8; 32]) -> Self {
        unsafe { core::mem::transmute(value) }
    }
}

impl Into<[u8; 32]> for Dirent {
    fn into(self) -> [u8; 32] {
        unsafe { core::mem::transmute(self) }
    }
}

#[repr(packed)]
#[derive(Debug, Default, Clone, Copy)]
/// FAT long file name directory entry.
pub struct LfnEnt {
    /// Order of this entry in the sequence of LFN entries.
    pub order: u8,
    /// Unicode characters 1-5 of this entry.
    pub name1: [u16; 5],
    /// File attributes.
    pub attr: u8,
    /// Must be set to 0.
    pub type_: u8,
    /// Checksum of the name in associated [`Dirent`].
    pub checksum: u8,
    /// Unicode characters 6-11 of this entry.
    pub name2: [u16; 6],
    /// Must be set to 0.
    pub first_cluster_lo: u16,
    /// Unicode characters 12-13 of this entry.
    pub name3: [u16; 2],
}
static_assertions::assert_eq_size!(LfnEnt, [u8; 32]);

#[rustfmt::skip]
impl LfnEnt {
    /// Converts all integers from little-endian.
    pub const fn from_le(&mut self) {
        self.name1[0]         = u16::from_le(self.name1[0]);
        self.name1[1]         = u16::from_le(self.name1[1]);
        self.name1[2]         = u16::from_le(self.name1[2]);
        self.name1[3]         = u16::from_le(self.name1[3]);
        self.name1[4]         = u16::from_le(self.name1[4]);
        self.name2[0]         = u16::from_le(self.name2[0]);
        self.name2[1]         = u16::from_le(self.name2[1]);
        self.name2[2]         = u16::from_le(self.name2[2]);
        self.name2[3]         = u16::from_le(self.name2[3]);
        self.name2[4]         = u16::from_le(self.name2[4]);
        self.name2[5]         = u16::from_le(self.name2[5]);
        self.name3[0]         = u16::from_le(self.name3[0]);
        self.name3[1]         = u16::from_le(self.name3[1]);
        self.first_cluster_lo = u16::from_le(self.first_cluster_lo);
    }

    /// Converts all integers into little-endian.
    pub const fn to_le(&mut self) {
        self.name1[0]         = u16::to_le(self.name1[0]);
        self.name1[1]         = u16::to_le(self.name1[1]);
        self.name1[2]         = u16::to_le(self.name1[2]);
        self.name1[3]         = u16::to_le(self.name1[3]);
        self.name1[4]         = u16::to_le(self.name1[4]);
        self.name2[0]         = u16::to_le(self.name2[0]);
        self.name2[1]         = u16::to_le(self.name2[1]);
        self.name2[2]         = u16::to_le(self.name2[2]);
        self.name2[3]         = u16::to_le(self.name2[3]);
        self.name2[4]         = u16::to_le(self.name2[4]);
        self.name2[5]         = u16::to_le(self.name2[5]);
        self.name3[0]         = u16::to_le(self.name3[0]);
        self.name3[1]         = u16::to_le(self.name3[1]);
        self.first_cluster_lo = u16::to_le(self.first_cluster_lo);
    }
}

impl LfnEnt {
    /// Maximum total number of unicode characters in a set of long name entries.
    pub const MAX_LEN: usize = 255;

    /// Get the unicode characters from this name entry.
    pub const fn get_name(&self) -> [u16; 13] {
        [
            self.name1[0],
            self.name1[1],
            self.name1[2],
            self.name1[3],
            self.name1[4],
            self.name2[0],
            self.name2[1],
            self.name2[2],
            self.name2[3],
            self.name2[4],
            self.name2[5],
            self.name3[0],
            self.name3[1],
        ]
    }

    /// Set the unicode characters in this name entry.
    pub const fn set_name(&mut self, name: &[u16; 13]) {
        self.name1[0] = name[0];
        self.name1[1] = name[1];
        self.name1[2] = name[2];
        self.name1[3] = name[3];
        self.name1[4] = name[4];
        self.name2[0] = name[5];
        self.name2[1] = name[6];
        self.name2[2] = name[7];
        self.name2[3] = name[8];
        self.name2[4] = name[9];
        self.name2[5] = name[10];
        self.name3[0] = name[11];
        self.name3[1] = name[12];
    }
}

impl Into<Dirent> for LfnEnt {
    fn into(self) -> Dirent {
        unsafe { core::mem::transmute(self) }
    }
}

impl From<[u8; 32]> for LfnEnt {
    fn from(value: [u8; 32]) -> Self {
        unsafe { core::mem::transmute(value) }
    }
}

impl Into<[u8; 32]> for LfnEnt {
    fn into(self) -> [u8; 32] {
        unsafe { core::mem::transmute(self) }
    }
}
