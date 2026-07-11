use alloc::{boxed::Box, string::String, vec::Vec};
use uuid::Uuid;

use crate::{
    bindings::error::EResult, dev2::class::block::BlockDevice, kernel::sync::mutex::Mutex,
};

pub mod gpt;
pub mod mbr;

/// Describes a single partition.
#[derive(Clone, Debug, Default)]
pub struct Partition {
    /// On-disk byte offset.
    pub offset: u64,
    /// On-disk byte size.
    pub size: u64,
    /// Type UUID.
    pub type_: Uuid,
    /// Partition UUID.
    pub uuid: Uuid,
    /// Partition name converted to UTF-8.
    pub name: String,
    /// Whether the partition is read-only.
    pub readonly: bool,
}

/// Describes the partitioning system on a particular volume.
#[derive(Clone, Debug, Default)]
pub struct VolumeInfo {
    /// Array of partitions.
    pub parts: Vec<Partition>,
    /// Volume label / name.
    pub name: String,
    /// Disk UUID converted to u128 with equivalent printed hex value.
    pub uuid: Uuid,
}

/// A partitioning system.
pub trait PartitionDriver {
    /// Detect this partitioning system on a medium and if present return the partitions.
    fn detect(&self, drive: &dyn BlockDevice) -> EResult<Option<VolumeInfo>>;
}

/// Set of partition system drivers.
pub static PARTITION_DRIVERS: Mutex<Vec<Box<dyn PartitionDriver>>> = Mutex::new(Vec::new());

/// Get the volume information for a particular drive.
pub fn get_volume_info(drive: &dyn BlockDevice) -> EResult<Option<VolumeInfo>> {
    drive.identify()?;
    for driver in &*PARTITION_DRIVERS.lock_shared()? {
        if let Some(data) = driver.detect(drive)? {
            return Ok(Some(data));
        }
    }
    Ok(None)
}
