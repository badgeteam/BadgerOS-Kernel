// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::{
    collections::btree_map::BTreeMap,
    string::String,
    sync::{Arc, Weak},
};

use crate::{
    bindings::error::{EResult, Errno},
    filesystem::File,
    kernel::sync::mutex::Mutex,
};

use super::{Device, class::block::BlockDevice};

static NODES: Mutex<BTreeMap<String, Nodes>> = Mutex::new(BTreeMap::new());

enum Nodes {
    Singleton(Weak<dyn Device>),
    Block(BTreeMap<u32, Weak<dyn BlockDevice>>),
    Generic(BTreeMap<u32, Weak<dyn Device>>),
}

/// Non-numbered devices; should only be used for special devices like `null` and `zero`.
pub fn add_singleton(name: &str, device: Arc<dyn Device>) -> EResult<()> {
    let mut nodes = NODES.unintr_lock();

    // Check that the name isn't in use already.
    if let Some(nodes) = nodes.get(name) {
        if let Nodes::Singleton(weak) = nodes {
            if weak.strong_count() > 0 {
                return Err(Errno::EEXIST);
            }
        } else {
            return Err(Errno::EEXIST);
        }
    }

    nodes.insert(name.into(), Nodes::Singleton(Arc::downgrade(&device)));

    Ok(())
}

/// Add a generic device node.
pub fn add(name: &str, device: Arc<dyn Device>) -> EResult<()> {
    todo!()
}

/// Add a block device node (e.g. `sataN` and `sataNpM`).
pub fn add_block(name: &str, device: Arc<dyn BlockDevice>) -> EResult<()> {
    todo!()
}

/// Populate a filesystem directory with the device nodes.
/// Fails if a conflicting name already exists for a different device handle.
pub fn populate(devnode_dir: &dyn File) -> EResult<()> {
    todo!()
}
