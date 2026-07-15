use core::panic;

use alloc::sync::Arc;
use uuid::Uuid;

use crate::{
    LogLevel,
    cpu::timer::time_us,
    dev2::{Device, class::block::BlockDevice, registry},
    kernel::sched::thread_sleep,
    misc::kparam,
    util,
};

use super::{
    media::{Media, MediaType},
    mount,
    partition::Partition,
};

pub static mut KFILE_GPT_DISK: Uuid = Uuid::nil();
pub static mut KFILE_GPT_PART: Uuid = Uuid::nil();
pub static mut KFILE_MBR_DISK: u32 = 0;

/// Find partition by GUID.
fn find_part_by_guid(guid: Uuid, is_type: bool) -> Option<(Arc<dyn BlockDevice>, Partition)> {
    let devs = registry::devices_by_trait::<dyn BlockDevice>().ok()?;
    for dev in devs {
        let _: Option<_> = try {
            let info = dev.volume_info(false).ok()??;
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
fn find_disk_by_guid(guid: Uuid) -> Option<Arc<dyn BlockDevice>> {
    let devs = registry::devices_by_trait::<dyn BlockDevice>().ok()?;
    for dev in devs {
        let _: Option<_> = try {
            let info = dev.volume_info(false).ok()??;
            if info.uuid == guid {
                return Some(dev);
            }
        };
    }
    None
}

/// Try to find the device that the kernel was loaded from.
fn find_kernel_disk() -> Option<Arc<dyn BlockDevice>> {
    unsafe {
        let gpt_disk = KFILE_GPT_DISK;
        let gpt_part = KFILE_GPT_PART;

        // Try to find disk by disk GUID.
        if !gpt_disk.is_nil()
            && let Some(res) = find_disk_by_guid(gpt_disk)
        {
            return Some(res);
        }

        // Try to find disk by MBR ID.
        if KFILE_MBR_DISK != 0
            && let Some(res) = find_disk_by_guid(Uuid::from_u128(KFILE_MBR_DISK as u128))
        {
            return Some(res);
        }

        // Try to find disk by partition GUID.
        if !gpt_part.is_nil()
            && let Some(res) = find_part_by_guid(gpt_part, true)
        {
            return Some(res.0);
        }

        // Unable to find kernel disk.
        None
    }
}

/// Try to find a disk by node name; <type><index>.
/// The matching block devices are sorted by ID.
fn find_disk_by_nodename(nodename: &str) -> Option<Arc<dyn BlockDevice>> {
    // TODO: dev2 has no device nodes yet.

    None
}

/// Filter applicable disks' partitions.
fn filter_parts(
    kernel_disk: Option<Arc<dyn BlockDevice>>,
    root_disk: Option<Arc<dyn BlockDevice>>,
    mut filter: impl FnMut(&Partition) -> bool,
) -> Option<(Arc<dyn BlockDevice>, Partition)> {
    // Collect devices to search from.
    let devs = if let Some(root_disk) = root_disk {
        vec![root_disk]
    } else {
        let mut devs = registry::devices_by_trait::<dyn BlockDevice>().ok()?;
        if let Some(kernel_disk) = kernel_disk {
            // This filter and reinsert here causes the kernel disk to be searched first, and only once.
            devs = devs
                .into_iter()
                .filter(|dev| (&**dev as &dyn Device).id() != (&*kernel_disk as &dyn Device).id())
                .collect();
            devs.insert(0, kernel_disk);
        }
        devs
    };

    for dev in devs {
        if let Ok(Some(info)) = dev.volume_info(false) {
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
    let timeout = try { kparam::get_kparam("ROOTWAIT")?.parse::<u32>().ok()? }.unwrap_or(5);
    let lim = time_us() + timeout as u64 * 1000000;

    while time_us() < lim {
        if mount_root_impl(false) {
            return;
        }
        let _ = thread_sleep(250000);
    }
    mount_root_impl(true);
}

pub fn mount_root_impl(do_panic: bool) -> bool {
    // Try to find the root disk.
    let kernel_disk = find_kernel_disk();
    let root_disk: Option<Arc<dyn BlockDevice>> = try {
        let param = kparam::get_kparam("ROOTDISK")?;
        let res: Option<_> = try {
            if param[..5] == *"UUID=" {
                find_disk_by_guid(util::parse_uuid_str(&param[5..])?)?
            } else {
                find_disk_by_nodename(param)?
            }
        };
        if res.is_none() {
            if do_panic {
                panic!("Unable to find ROOTDISK={}", param);
            } else {
                return false;
            }
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
                let info = root_disk.volume_info(false).ok()??;
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
            if do_panic {
                panic!("Unable to find kernel disk needed to mount root");
            } else {
                return false;
            }
        }
    } else {
        panic!("Unknown format for ROOT={}", param);
    };

    if part.is_none() {
        if do_panic {
            panic!("Unable to find ROOT={}", param);
        } else {
            return false;
        }
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
        (&*disk as &dyn Device).id(),
        offset,
        size
    );

    // Finally mount filesystem.
    let res = mount::mount(None, b"/", None, Some(media), 0);
    if let Err(x) = res {
        panic!("Unable to mount root filesystem: {}", x);
    }

    true
}
