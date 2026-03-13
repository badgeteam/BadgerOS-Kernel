use core::mem::MaybeUninit;

use alloc::{string::String, vec::Vec};
use bytemuck::{AnyBitPattern, NoUninit, Zeroable, cast_slice_mut};
use uuid::Uuid;

use crate::{
    LogLevel,
    bindings::{device::class::block::BlockDevice, error::EResult},
    filesystem::partition::{Partition, PartitionDriver, VolumeInfo, mbr::MbrDriver},
    util,
};
use crc::{CRC_32_ISO_HDLC, Crc};

/// GPT Partition table header.
#[repr(packed)]
#[derive(Default, Clone, Copy)]
pub struct GptHeader {
    /// GPT signature.
    pub signature: [u8; 8],
    /// GPT revision.
    pub revision: u32,
    /// Header size (specifies the range on which a checksum should be calculated).
    pub size: u32,
    /// CRC32 checksum of the header.
    pub crc32: u32,
    /// Reserved (should be 0).
    pub resvd0: u32,
    /// The LBA of this header.
    pub this_lba: u64,
    /// The LBA of the other header.
    pub alt_lba: u64,
    /// First usable block.
    pub first_usable: u64,
    /// Last usable block.
    pub last_usable: u64,
    /// Disk UUID.
    pub uuid: [u8; 16],
    /// Starting LBA of partition entry array.
    pub parts_lba: u64,
    /// Number of partition entries.
    pub parts_len: u32,
    /// Size of each partition entry (BadgerOS will accept any number large enough).
    pub part_ent_size: u32,
    /// CRC32 of the partition entry array.
    pub parts_crc32: u32,
}
static_assertions::assert_eq_size!(GptHeader, [u8; 0x5c]);
unsafe impl Zeroable for GptHeader {}
unsafe impl NoUninit for GptHeader {}
unsafe impl AnyBitPattern for GptHeader {}

impl GptHeader {
    /// Value that the signature bytes must have for the GPT header.
    pub const GPT_SIGNATURE: [u8; 8] = *b"EFI PART";
}

pub struct GptDriver {}

impl GptDriver {
    /// Initializer for GPT CRC32.
    pub const GPT_CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

    /// Helper function that calculates the GPT header CRC32.
    pub fn gpt_header_crc32(drive: BlockDevice, header_lba: u64, header_len: u32) -> EResult<u32> {
        #[allow(invalid_value)]
        let mut buf: [u8; 512] = unsafe { MaybeUninit::uninit().assume_init() };
        drive.readk_bytes(
            header_lba << drive.block_size_exp(),
            &mut buf[..header_len as usize],
        )?;

        // Clear out the CRC32 checksum field to 0 for correct calculation.
        buf[0x10] = 0;
        buf[0x11] = 0;
        buf[0x12] = 0;
        buf[0x13] = 0;

        Ok(Self::GPT_CRC32.checksum(&buf[..header_len as usize]))
    }

    /// Helper function that tries to get a GPT header.
    pub fn get_gpt_header(drive: BlockDevice, header_lba: u64) -> EResult<Option<GptHeader>> {
        let mut raw_header = [GptHeader::default()];
        drive.readk_bytes(
            header_lba << drive.block_size_exp(),
            cast_slice_mut(&mut raw_header),
        )?;
        let raw_header = raw_header[0];
        if raw_header.signature != GptHeader::GPT_SIGNATURE {
            logkf!(
                LogLevel::Error,
                "Expected GPT header at block {}; ignoring this header",
                header_lba
            );
            return Ok(None);
        }
        if raw_header.size as usize > 512 || raw_header.size < 0x5c {
            logkf!(
                LogLevel::Error,
                "GPT header length {} is invalid; ignoring this header",
                0 + raw_header.size
            );
            return Ok(None);
        }

        let crc = Self::gpt_header_crc32(drive.clone(), header_lba, raw_header.size)?;
        if raw_header.crc32 != crc {
            logkf!(
                LogLevel::Error,
                "GPT CRC32 mismatch (expected 0x{:08x}, calculated 0x{:08x}; ignoring this header",
                0 + raw_header.crc32,
                crc
            );
            return Ok(None);
        }

        if raw_header.this_lba != header_lba {
            logkf!(
                LogLevel::Error,
                "GPT header at block {} thinks it's at {}; ignoring this header",
                header_lba,
                0 + raw_header.this_lba
            );
            return Ok(None);
        }

        if header_lba == 1 && raw_header.alt_lba != drive.block_count() - 1 {
            logkf!(
                LogLevel::Warning,
                "Primary GPT header thinks alternate header is not at the end of the disk"
            );
        } else if header_lba != 1 && raw_header.alt_lba != 1 {
            logkf!(
                LogLevel::Warning,
                "Alternate GPT header thinks primary header is not at the start of the disk"
            );
        }

        Ok(Some(raw_header))
    }

    /// Read a single GPT partition entry.
    pub fn get_partition(
        drive: BlockDevice,
        part_ent_offset: u64,
        part_ent_len: u32,
    ) -> EResult<Option<Partition>> {
        let mut type_ = [0u8; 16];
        drive.readk_bytes(part_ent_offset, &mut type_)?;
        let type_ = Uuid::from_bytes_le(type_);
        if type_.as_u128() == 0 {
            return Ok(None);
        }

        let mut uuid = [0u8; 16];
        drive.readk_bytes(part_ent_offset + 16, &mut uuid)?;
        let uuid = Uuid::from_bytes_le(uuid);

        let mut offset = [0u8; 8];
        drive.readk_bytes(part_ent_offset + 32, &mut offset)?;
        let offset = u64::from_le_bytes(offset) << drive.block_size_exp();

        let mut size = [0u8; 8];
        drive.readk_bytes(part_ent_offset + 40, &mut size)?;
        let size = u64::from_le_bytes(size) << drive.block_size_exp();

        // Attributes field ignored.

        let max_name_len = part_ent_len as usize - 56;
        let mut name = Vec::<u8>::try_with_capacity(max_name_len)?;
        name.resize(max_name_len, 0);
        drive.readk_bytes(part_ent_offset + 56, &mut name)?;
        let name = util::parse_utf16_le(&name)?;

        Ok(Some(Partition {
            offset,
            size,
            type_,
            uuid,
            name,
            readonly: false,
        }))
    }
}

impl PartitionDriver for GptDriver {
    fn detect(&self, drive: BlockDevice) -> EResult<Option<VolumeInfo>> {
        // Read the GPT headers.
        let first_gpt = Self::get_gpt_header(drive.clone(), 1)?;
        let second_gpt = if let Some(first_gpt) = first_gpt {
            Self::get_gpt_header(drive.clone(), first_gpt.alt_lba)?
        } else {
            Self::get_gpt_header(drive.clone(), drive.block_count() - 1)?
        };

        // Decide which GPT header to use.
        let active_gpt = if let Some(gpt) = first_gpt.or_else(|| second_gpt) {
            gpt
        } else {
            // No valid GPT headers; assume this isn't a GPT disk.
            return Ok(None);
        };

        // Check whather the protective MBR is present.
        if let Some(mbr) = MbrDriver::detect_nopgt(drive.clone())? {
            if !MbrDriver::is_protective_mbr(&mbr) {
                logkf!(
                    LogLevel::Error,
                    "Apparently GPT-formatted disk has an MBR that is non-protective; assuming it to be an MBR disk instead"
                );
                return Ok(Some(mbr));
            }
        } else {
            logkf!(
                LogLevel::Warning,
                "GPT-formatted disk is missing protective MBR"
            );
        }

        // Read all the partition entries.
        let mut parts = Vec::try_with_capacity(active_gpt.parts_len as usize)?;
        for i in 0..active_gpt.parts_len {
            let part_ent_offset = (active_gpt.parts_lba << drive.block_size_exp())
                + i as u64 * active_gpt.part_ent_size as u64;
            if let Some(part) =
                Self::get_partition(drive.clone(), part_ent_offset, active_gpt.part_ent_size)?
            {
                parts.push(part);
            }
        }

        Ok(Some(VolumeInfo {
            parts,
            name: String::new(),
            uuid: Uuid::from_bytes_le(active_gpt.uuid),
        }))
    }
}
