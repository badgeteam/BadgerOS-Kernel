use core::{ffi::c_void, ops::Deref, ptr::NonNull};

use alloc::vec::Vec;
use uuid::Uuid;

use crate::{
    LogLevel,
    bindings::{
        device::{DeviceFilters, HasBaseDevice, class::block::BlockDevice, iter_drivers},
        raw::{
            dev_class_t_DEV_CLASS_BLOCK, driver_block_t, driver_t, limine_kernel_file_request,
            limine_uuid, mem_equals, strlen,
        },
    },
    kparam, util,
};

use super::{
    media::{Media, MediaType},
    mount,
    partition::{Partition, get_volume_info},
};

unsafe extern "C" {
    #[link_name = "bootp_kernel_file_req"]
    static KERNEL_FILE: limine_kernel_file_request;
}

/// Helper function that converts Limine UUID to u128.
const fn limine_uuid_conv(uuid: limine_uuid) -> Uuid {
    Uuid::from_fields(uuid.a, uuid.b, uuid.c, &uuid.d)
}

/// Find partition by GUID.
fn find_part_by_guid(guid: Uuid, is_type: bool) -> Option<(BlockDevice, Partition)> {
    let devs = BlockDevice::filter(DeviceFilters::default()).ok()?;
    for dev in devs {
        let _: Option<_> = try {
            let info = get_volume_info(dev.clone()).ok()??;
            for part in info.parts {
                if if is_type { part.type_ } else { part.uuid } == guid {
                    return Some((dev, part));
                }
            }
        };
    }
    None
}

/// Find disk by GUID.
fn find_disk_by_guid(guid: Uuid) -> Option<BlockDevice> {
    let devs = BlockDevice::filter(DeviceFilters::default()).ok()?;
    for dev in devs {
        let _: Option<_> = try {
            let info = get_volume_info(dev.clone()).ok()??;
            if info.uuid == guid {
                return Some(dev);
            }
        };
    }
    None
}

/// Try to find the device that the kernel was loaded from.
fn find_kernel_disk() -> Option<BlockDevice> {
    let kernel_file = unsafe {
        if KERNEL_FILE.response.is_null() {
            return None;
        }
        &*(*KERNEL_FILE.response).kernel_file
    };

    // Try to find disk by disk GUID.
    if limine_uuid_conv(kernel_file.gpt_disk_uuid).as_u128() != 0
        && let Some(res) = find_disk_by_guid(limine_uuid_conv(kernel_file.gpt_disk_uuid))
    {
        return Some(res);
    }

    // Try to find disk by MBR ID.
    if kernel_file.mbr_disk_id != 0
        && let Some(res) = find_disk_by_guid(Uuid::from_u128(kernel_file.mbr_disk_id as u128))
    {
        return Some(res);
    }

    // Try to find disk by partition GUID.
    if limine_uuid_conv(kernel_file.gpt_part_uuid).as_u128() != 0
        && let Some(res) = find_part_by_guid(limine_uuid_conv(kernel_file.gpt_part_uuid), true)
    {
        return Some(res.0);
    }

    // Unable to find kernel disk.
    None
}

/// Try to find a disk by node name; <type><index>.
/// The matching block devices are sorted by ID.
fn find_disk_by_nodename(nodename: &str) -> Option<BlockDevice> {
    nodename.as_ascii()?;

    // Extract the class from the nodename.
    let class_len = nodename
        .chars()
        .into_iter()
        .position(|x| x >= '0' && x <= '9')?;
    let class = &nodename[..class_len];

    // Extract the index from the nodename.
    let index = nodename[class_len..].parse::<u32>().ok()?;

    // Find block devices of matching driver.
    let mut driver = None;
    iter_drivers(|guard| {
        if guard.dev_class == dev_class_t_DEV_CLASS_BLOCK {
            let as_block = unsafe { &*(guard.deref() as *const driver_t as *const driver_block_t) };
            let strlen = unsafe { strlen(as_block.blk_node_name) };
            if strlen == class.len()
                && unsafe {
                    mem_equals(
                        class.as_ptr() as *const c_void,
                        as_block.blk_node_name as *const c_void,
                        strlen,
                    )
                }
            {
                driver = Some(guard);
                false
            } else {
                true
            }
        } else {
            true
        }
    });
    let driver = driver?;

    // Get all devices with matching node name.
    let devs = BlockDevice::filter(DeviceFilters {
        driver: Some(NonNull::from(driver.deref())),
        ..Default::default()
    })
    .ok()?;
    let mut devs: Vec<_> = devs.iter().collect();
    devs.sort_by(|a, b| a.id().cmp(&b.id()));

    // Look up the device from this array.
    if (index as usize) < devs.len() {
        Some(devs[index as usize].clone())
    } else {
        None
    }
}

/// Filter applicable disks' partitions.
fn filter_parts(
    kernel_disk: Option<BlockDevice>,
    root_disk: Option<BlockDevice>,
    mut filter: impl FnMut(&Partition) -> bool,
) -> Option<(BlockDevice, Partition)> {
    // Collect devices to search from.
    let devs = if let Some(root_disk) = root_disk {
        vec![root_disk]
    } else {
        let mut devs: Vec<_> = BlockDevice::filter(Default::default())
            .ok()?
            .into_iter()
            .filter(|dev| {
                kernel_disk
                    .as_ref()
                    .map(|x| x.id() != dev.id())
                    .unwrap_or(true)
            })
            .collect();
        if let Some(kernel_disk) = kernel_disk {
            devs.insert(0, kernel_disk);
        }
        devs
    };

    for dev in devs {
        if let Ok(Some(info)) = get_volume_info(dev.clone()) {
            for part in info.parts {
                if filter(&part) {
                    return Some((dev, part));
                }
            }
        }
    }

    None
}

/// Mount the root filesystem according to kernel parameters.
pub fn mount_root_fs() {
    // Try to find the root disk.
    let kernel_disk = find_kernel_disk();
    let root_disk: Option<BlockDevice> = try {
        let param = kparam::get_kparam("ROOTDISK")?;
        let res: Option<_> = try {
            if param[..5] == *"UUID=" {
                find_disk_by_guid(util::parse_uuid_str(&param[5..])?)?
            } else {
                find_disk_by_nodename(param)?
            }
        };
        if res.is_none() {
            panic!("Unable to find ROOTDISK={}", param);
        }
        res?
    };

    // Try to find the root partition.
    let param =
        kparam::get_kparam("ROOT").unwrap_or("PARTTYPE=0FC63DAF-8483-4772-8E79-3D69D8477DE4");
    let part = if param.len() >= 10 && param[..9] == *"PARTUUID=" {
        // Partition by UUID.
        if let Some(uuid) = util::parse_uuid_str(&param[9..]) {
            filter_parts(kernel_disk, root_disk, |part| part.uuid == uuid)
                .map(|(dev, part)| (dev, Some(part)))
        } else {
            None
        }
    } else if param.len() >= 10 && param[..9] == *"PARTTYPE=" {
        // Partition by type.
        if let Some(uuid) = util::parse_uuid_str(&param[9..]) {
            filter_parts(kernel_disk, root_disk, |part| part.type_ == uuid)
                .map(|(dev, part)| (dev, Some(part)))
        } else {
            None
        }
    } else if param.len() >= 6 && param[..5] == *"PART=" {
        // Partition indexed into the root disk.
        // By default, use the kernel disk.
        if let Some(root_disk) = if root_disk.is_none() {
            &kernel_disk
        } else {
            &root_disk
        } {
            try {
                let info = get_volume_info(root_disk.clone()).ok()??;
                let index = param[5..].parse::<usize>().ok()?;
                (index < info.parts.len())
                    .then(|| (root_disk.clone(), Some(info.parts[index].clone())))?
            }
        } else {
            logkf!(
                LogLevel::Fatal,
                "Unable to find kernel disk needed to mount root"
            );
            None
        }
    } else if *param == *"WHOLEDISK" {
        // Use the whole root disk to mount the filesystem.
        if let Some(root_disk) = if root_disk.is_none() {
            kernel_disk
        } else {
            root_disk
        } {
            Some((root_disk, None))
        } else {
            logkf!(
                LogLevel::Fatal,
                "Unable to find kernel disk needed to mount root"
            );
            None
        }
    } else {
        panic!("Unknown format for ROOT={}", param);
    };

    if part.is_none() {
        panic!("Unable to find ROOT={}", param);
    }
    let (disk, part) = part.unwrap();

    // Convert to filesystem media.
    let (offset, size) = if let Some(part) = part {
        (part.offset, part.size)
    } else {
        (0u64, disk.block_count() << disk.block_size_exp())
    };
    let media = Media {
        offset,
        size,
        storage: MediaType::Block(disk.clone()),
    };

    logkf!(
        LogLevel::Info,
        "Mounting root filesystem on blkdev {}; offset 0x{:x}, size 0x{:x}",
        disk.id(),
        offset,
        size
    );

    // Finally mount filesystem.
    let res = mount(None, b"/", None, Some(media), 0);
    if let Err(x) = res {
        panic!("Unable to mount root filesystem: {}", x);
    }
}
