// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    device::class::block::BlockDevice,
    filesystem::{
        File, MakeFileSpec, VfsLoc, make_file,
        mount::{self, Mount},
        oflags, ramfs, unlink,
        vfs::{VNode, Vfs, VfsFile},
    },
};

use super::Device;

static mut INSTANCE: Option<(Arc<Vfs>, Arc<VNode>)> = None;
static mut VFS_LOC: Option<VfsLoc> = None;

/// Get the devtmpfs filesystem instance and its root vnode.
pub fn instance() -> (Arc<Vfs>, Arc<VNode>) {
    unsafe {
        (*&raw const INSTANCE)
            .clone()
            .expect("devtmpfs not initialized")
    }
}

/// Get a directory handle for the root of the devtmpfs.
pub fn handle() -> Arc<dyn File> {
    let loc = unsafe {
        (*&raw const VFS_LOC)
            .clone()
            .expect("devtmpfs not initialized")
    };
    Arc::new(VfsFile::new(loc, oflags::READ_ONLY))
}

/// Create a node for a device.
pub(super) fn create_node(device: Arc<dyn Device>, name: &str, is_singleton: bool) -> EResult<u32> {
    let handle = handle();

    let id = device.id();
    let spec;
    if let Some(device) = device.clone().try_as_arc::<dyn BlockDevice>() {
        spec = MakeFileSpec::BlockDev(device);
    } else {
        spec = MakeFileSpec::CharDev(device);
    }

    if is_singleton {
        return make_file(Some(&*handle), name.as_bytes(), spec).map(|_| 0);
    }

    for i in 0..u32::MAX {
        let path = format!("{}{}", name, i);
        match make_file(Some(&*handle), path.as_bytes(), spec.clone()) {
            Ok(_) => {
                logkf!(
                    LogLevel::Info,
                    "Create devtmpfs node {} for device {}",
                    path,
                    id
                );
                return Ok(i);
            }
            Err(Errno::EEXIST) => (),
            Err(x) => return Err(x),
        }
    }

    // We're out of device IDs long before this happens.
    unreachable!()
}

/// Remove a node for a device.
/// Also remove all nodes that are prefixed by this node's name,
/// assuming it's undesirable that unregistered devices' children are still accessible.
pub(super) fn remove_node(name: &str, index: Option<u32>) {
    let handle = handle();

    let prefix;
    if let Some(index) = index {
        prefix = format!("{}{}", name, index);
    } else {
        prefix = name.into();
    }

    let mut dents = Vec::new();
    if let Err(x) = handle.get_dirents(&mut dents) {
        logkf!(LogLevel::Warning, "Cannot remove from devtmpfs: {}", x);
        return;
    }

    for dent in dents {
        if dent.name.starts_with(prefix.as_bytes()) {
            match unlink(Some(&*handle), &dent.name, false) {
                Ok(_) => logkf!(
                    LogLevel::Info,
                    "Remove devtmpfs node {}",
                    String::from_utf8_lossy(&dent.name)
                ),
                Err(x) => logkf!(LogLevel::Warning, "Cannot remove from devtmpfs: {}", x),
            }
        }
    }
}

/// Create the devtmpfs.
pub(super) unsafe fn init() {
    let ops = ramfs::RamFs::new(true).expect("Failed to create RamFs for devtmpfs");
    let (vfs, root) = mount::create_vfs(Box::new(ops)).expect("Failed to create Vfs for devtmpfs");
    let loc = VfsLoc {
        vnode: root.clone(),
        mount: Arc::new(Mount::new(None, root.clone(), vfs.clone())),
    };
    unsafe {
        (&raw mut INSTANCE).write(Some((vfs, root)));
        (&raw mut VFS_LOC).write(Some(loc));
    }
}
