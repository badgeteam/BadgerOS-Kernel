use alloc::{string::String, vec::Vec};
use bytemuck::{AnyBitPattern, NoUninit, Zeroable, cast_slice_mut};
use uuid::Uuid;

use crate::{
    bindings::{device::class::block::BlockDevice, error::EResult},
    filesystem::partition::{Partition, PartitionDriver, VolumeInfo},
};

/// Cylinder, head, sector address.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ChsAddr([u8; 3]);

impl Default for ChsAddr {
    fn default() -> Self {
        Self([0; 3])
    }
}

/// MBR partition table entry.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct MbrEntry {
    /// Drive attributes bitmask.
    pub attr: u8,
    /// CHS address of start of partition; not used by BadgerOS.
    pub chs_start: ChsAddr,
    /// Partition type.
    pub type_: u8,
    /// CHS address of end of partition; not used by BadgerOS.
    pub chs_end: ChsAddr,
    /// LBA address of start of partition.
    pub lba_start: u32,
    /// Partition size in sectors.
    pub sec_count: u32,
}
unsafe impl Zeroable for MbrEntry {}
unsafe impl NoUninit for MbrEntry {}
unsafe impl AnyBitPattern for MbrEntry {}

pub struct MbrDriver {}

impl MbrDriver {
    /// Detect but do not exclude GPT-formatted drives.
    pub fn detect_nopgt(drive: BlockDevice) -> EResult<Option<VolumeInfo>> {
        // Look for signature bytes.
        let mut signature = [0u8; 2];
        drive.readk_bytes(0x1fe, &mut signature)?;
        if signature != [0x55, 0xaa] {
            return Ok(None);
        }

        // Read the reserved bytes, as they can indicate read-only disks.
        let mut readonly_marker = [0u8; 2];
        drive.readk_bytes(0x1bc, &mut readonly_marker)?;
        let glob_readonly = readonly_marker == [0x5a, 0x5a];

        // Read raw MBR entries.
        let mut raw_parts: [MbrEntry; 4] = Default::default();
        drive.readk_bytes(0x1be, cast_slice_mut(&mut raw_parts))?;

        // Convert individual partitions.
        let mut parts = Vec::new();
        for part in raw_parts {
            if part.lba_start != 0 && part.sec_count != 0 {
                parts.push(Partition {
                    offset: (part.lba_start as u64) << drive.block_size_exp(),
                    size: (part.sec_count as u64) << drive.block_size_exp(),
                    type_: Uuid::from_u128(part.type_ as u128),
                    uuid: Uuid::from_u128(0),
                    name: String::new(),
                    readonly: glob_readonly,
                });
            }
        }

        if parts.len() == 0 {
            // No partitions in the MBR.
            return Ok(None);
        }

        Ok(Some(VolumeInfo {
            parts,
            name: String::new(),
            uuid: Uuid::from_u128(0),
        }))
    }

    /// Test whether a volume info, assuming it is MBR, is the protective MBR for a GPT disk.
    pub fn is_protective_mbr(info: &VolumeInfo) -> bool {
        info.parts.len() == 1 && info.parts[0].type_.as_u128() == 0xee
    }
}

impl PartitionDriver for MbrDriver {
    fn detect(&self, drive: BlockDevice) -> EResult<Option<VolumeInfo>> {
        let res = Self::detect_nopgt(drive);

        if let Ok(Some(info)) = &res {
            if Self::is_protective_mbr(info) {
                return Ok(None);
            }
        }

        res
    }
}
