// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};

use crate::{
    bindings::error::{EResult, Errno},
    filesystem::{self, File, oflags, sysimpl::AT_FDCWD},
    process::FILE_MAX,
};

/// A single file descriptor.
pub struct FileDesc {
    pub flags: AtomicU32,
    pub file: Arc<dyn File>,
}
impl Clone for FileDesc {
    fn clone(&self) -> Self {
        Self {
            flags: AtomicU32::new(self.flags.load(Ordering::Relaxed)),
            file: self.file.clone(),
        }
    }
}

/// Process file descriptor table.
#[derive(Clone)]
pub struct FDTable {
    pub inner: BTreeMap<i32, FileDesc>,
    /// Absolute path to working directory.
    pub cwd: Vec<u8>,
    /// File handle that points to working directory.
    pub fd_cwd: Option<Arc<dyn File>>,
}

pub mod fdflags {
    pub const O_CLOEXEC: u32 = 0x0001_0000;

    pub const O_NOCTTY: u32 = 0x0100_0000;
}

impl FDTable {
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
            cwd: (*b"/").into(),
            fd_cwd: None,
        }
    }

    /// Change directory to that of a file descriptor.
    pub fn fchdir(&mut self, file: Arc<dyn File>) -> EResult<()> {
        // Note: This call also enforces the file handle is that of a directory.
        let path = filesystem::realpath(Some(file.as_ref()), b".", false)?;
        self.cwd = path;
        self.fd_cwd = Some(file);
        Ok(())
    }

    /// Change directory with a relative path.
    pub fn chdir(&mut self, path: &[u8]) -> EResult<()> {
        let cwd = filesystem::abspath(&self.cwd, path)?;
        let fd_cwd = Some(filesystem::open(
            self.fd_cwd.as_deref(),
            path,
            oflags::DIR_ONLY,
        )?);

        self.cwd = cwd;
        self.fd_cwd = fd_cwd;
        Ok(())
    }

    /// If `fileno` is [`AT_FDCWD`], return `self.fd_cwd`; otherwise, the same as [`Self::get_file`].
    pub fn get_atfile(&self, fileno: i32) -> EResult<Option<Arc<dyn File>>> {
        if fileno == AT_FDCWD {
            Ok(self.fd_cwd.clone())
        } else {
            Ok(Some(self.get_file(fileno)?))
        }
    }

    /// Get a file from the file descriptor table.
    pub fn get_file(&self, fileno: i32) -> EResult<Arc<dyn File>> {
        self.inner
            .get(&fileno)
            .map(|f| f.file.clone())
            .ok_or(Errno::EBADF)
    }

    /// Replace a file descriptor entry.
    pub fn replace_file(&mut self, fileno: i32, file: FileDesc) -> EResult<()> {
        if fileno < 0 || fileno >= FILE_MAX {
            return Err(Errno::EMFILE);
        }
        self.inner
            .insert(fileno, file)
            .map(|_| ())
            .ok_or(Errno::EBADF)
    }

    /// Insert a file into an empty slot of the file descriptor table.
    pub fn insert_file(&mut self, minfd: i32, file: FileDesc) -> EResult<i32> {
        let mut fileno = Err(Errno::EMFILE);
        for i in minfd..FILE_MAX {
            if !self.inner.contains_key(&i) {
                fileno = Ok(i);
                break;
            }
        }
        let fileno = fileno?;
        self.inner.insert(fileno, file);
        Ok(fileno)
    }

    /// Insert two files into an empty slot of the file descriptor table.
    pub fn insert_dual_file(&mut self, file0: FileDesc, file1: FileDesc) -> EResult<(i32, i32)> {
        let mut fileno0 = Err(Errno::EMFILE);
        let mut fileno1 = Err(Errno::EMFILE);
        for i in 0..FILE_MAX {
            if !self.inner.contains_key(&i) {
                if fileno0.is_err() {
                    fileno0 = Ok(i);
                } else {
                    fileno1 = Ok(i);
                    break;
                }
            }
        }

        let fileno0 = fileno0?;
        let fileno1 = fileno1?;

        self.inner.insert(fileno0, file0);
        self.inner.insert(fileno1, file1);

        Ok((fileno0, fileno1))
    }

    /// Remove a file descriptor from the file descriptor table.
    pub fn remove_file(&mut self, fileno: i32) -> EResult<()> {
        self.inner.remove(&fileno).map(|_| ()).ok_or(Errno::EBADF)
    }

    /// Helper that removes all [`O_CLOEXEC`] files.
    pub fn close_cloexec(&mut self) {
        self.inner
            .retain(|_, v| v.flags.load(Ordering::Relaxed) & fdflags::O_CLOEXEC == 0);
    }

    /// Clear this files table.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}
