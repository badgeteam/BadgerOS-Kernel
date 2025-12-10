// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::{c_char, c_int, c_long, c_void};

use crate::{
    bindings::{error::Errno, raw::stat_t},
    filesystem::{self, PATH_MAX},
    process::{
        self,
        usercopy::{self, UserPtrMut, UserSlice, UserSliceMut},
    },
};

use super::{MakeFileSpec, link, make_file, oflags, open, pipe, rename, unlink};

pub const AT_FDCWD: i32 = -100;

/// Open a file, optionally relative to a directory.
/// If `at` is -1, `path` is relative to the working directory.
/// Returns -errno on error, file descriptor number on success.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_open(at: c_int, path: *const c_char, oflags: c_int) -> c_int {
    let proc = process::current().unwrap();
    Errno::extract_i32(
        try {
            let mut pathbuf = [0u8; PATH_MAX];
            usercopy::read_user_cstr(path, &mut pathbuf)?;
            let at_file = proc.get_atfile(at)?;
            let file = filesystem::open(at_file.as_deref(), &pathbuf, oflags as u32)?;
            proc.insert_file(file)?
        },
    )
}

/// Flush and close a file.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_close(fd: c_int) -> c_int {
    let proc = process::current().unwrap();
    Errno::extract(proc.remove_file(fd as i32))
}

/// Read bytes from a file.
/// Returns 0 on EOF, -errno on error, read count on success.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_read(fd: c_int, read_buf: *mut c_void, read_len: c_long) -> c_long {
    if read_len < 0 {
        return -(Errno::EINVAL as c_long);
    }
    let proc = process::current().unwrap();
    Errno::extract_usize(
        try {
            proc.get_file(fd)?.read(UserSliceMut::new_mut(
                read_buf as *mut u8,
                read_len as usize,
            )?)?
        },
    ) as c_long
}

/// Write bytes to a file.
/// Returns -errno on error, write count on success.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_write(
    fd: c_int,
    write_buf: *const c_void,
    write_len: c_long,
) -> c_long {
    if write_len < 0 {
        return -(Errno::EINVAL as c_long);
    }
    let proc = process::current().unwrap();
    Errno::extract_usize(
        try {
            proc.get_file(fd)?
                .write(UserSlice::new(write_buf as *const u8, write_len as usize)?)?
        },
    ) as c_long
}

/// Read directory entries from a directory handle.
/// See `dirent_t` for the format.
/// Returns -errno on error, read count on success.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_getdents(
    fd: c_int,
    read_buf: *mut c_void,
    read_len: c_long,
) -> c_long {
    todo!()
}

/// Rename and/or move a file to another path, optionally relative to one or two directories.
/// If `*_at` is -1, the respective `*_path` is relative to the working directory.
/// Returns -errno on error, 0 on success.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_rename(
    old_at: c_int,
    old_path: *const c_char,
    new_at: c_int,
    new_path: *const c_char,
    flags: u32,
) -> c_int {
    let proc = process::current().unwrap();
    Errno::extract(
        try {
            let old_at_file = proc.get_atfile(old_at)?;
            let mut old_pathbuf = [0u8; PATH_MAX];
            usercopy::read_user_cstr(old_path, &mut old_pathbuf)?;
            let new_at_file = proc.get_atfile(new_at)?;
            let mut new_pathbuf = [0u8; PATH_MAX];
            usercopy::read_user_cstr(new_path, &mut new_pathbuf)?;
            rename(
                old_at_file.as_deref(),
                &old_pathbuf,
                new_at_file.as_deref(),
                &new_pathbuf,
                flags,
            )?;
        },
    ) as c_int
}

/// Get file status given file handler or path, optionally following the final symlink.
/// If `path` is specified, it is interpreted as relative to `fd`.
/// If `path` is NULL, the inode referenced by `fd` is stat'ed.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_stat(
    fd: c_int,
    path: *const c_char,
    follow_link: bool,
    stat_out: *mut stat_t,
) -> c_int {
    let proc = process::current().unwrap();
    Errno::extract(
        try {
            let mut stat_out = UserPtrMut::new_mut(stat_out)?;
            let stat: stat_t;
            if path.is_null() {
                stat = proc.get_file(fd)?.stat()?.into();
            } else {
                let mut pathbuf = [0u8; PATH_MAX];
                usercopy::read_user_cstr(path, &mut pathbuf)?;
                stat = open(
                    proc.get_atfile(fd)?.as_deref(),
                    &pathbuf,
                    if follow_link { 0 } else { oflags::NOFOLLOW },
                )?
                .stat()?
                .into();
            }
            stat_out.write(stat)?;
        },
    ) as c_int
}

/// Create a new directory.
/// If `at` is -1, `path` is relative to the working directory.
/// Returns -errno on error, 0 on success.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_mkdir(at: c_int, path: *const c_char) -> c_int {
    let proc = process::current().unwrap();
    Errno::extract(
        try {
            let mut pathbuf = [0u8; PATH_MAX];
            usercopy::read_user_cstr(path, &mut pathbuf)?;
            make_file(
                proc.get_atfile(at)?.as_deref(),
                &pathbuf,
                MakeFileSpec::Directory,
            )?;
        },
    )
}

/// Delete a directory if it is empty.
/// If `at` is -1, `path` is relative to the working directory.
/// Returns -errno on error, 0 on success.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_rmdir(at: c_int, path: *const c_char) -> c_int {
    let proc = process::current().unwrap();
    Errno::extract(
        try {
            let mut pathbuf = [0u8; PATH_MAX];
            usercopy::read_user_cstr(path, &mut pathbuf)?;
            unlink(proc.get_atfile(at)?.as_deref(), &pathbuf, true)?;
        },
    )
}

/// Create a new link to an existing inode.
/// If `*_at` is -1, the respective `*_path` is relative to the working directory.
/// Returns -errno on error, 0 on success.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_link(
    old_at: c_int,
    old_path: *const c_char,
    new_at: c_int,
    new_path: *const c_char,
    flags: u32,
) -> c_int {
    let proc = process::current().unwrap();
    Errno::extract(
        try {
            let old_at_file = proc.get_atfile(old_at)?;
            let mut old_pathbuf = [0u8; PATH_MAX];
            usercopy::read_user_cstr(old_path, &mut old_pathbuf)?;
            let new_at_file = proc.get_atfile(new_at)?;
            let mut new_pathbuf = [0u8; PATH_MAX];
            usercopy::read_user_cstr(new_path, &mut new_pathbuf)?;
            link(
                old_at_file.as_deref(),
                &old_pathbuf,
                new_at_file.as_deref(),
                &new_pathbuf,
                flags,
            )?;
        },
    ) as c_int
}

/// Remove a link to an inode. If it is the last link, the file is deleted.
/// If `at` is -1, `path` is relative to the working directory.
/// Returns -errno on error, 0 on success.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_unlink(at: c_int, path: *const c_char) -> c_int {
    let proc = process::current().unwrap();
    Errno::extract(
        try {
            let mut pathbuf = [0u8; PATH_MAX];
            usercopy::read_user_cstr(path, &mut pathbuf)?;
            unlink(proc.get_atfile(at)?.as_deref(), &pathbuf, false)?;
        },
    )
}

/// Create a new FIFO / named pipe.
/// If `at` is -1, `path` is relative to the working directory.
/// Returns -errno on error, 0 on success.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_mkfifo(at: c_int, path: *const c_char) -> c_int {
    let proc = process::current().unwrap();
    Errno::extract(
        try {
            let mut pathbuf = [0u8; PATH_MAX];
            usercopy::read_user_cstr(path, &mut pathbuf)?;
            make_file(
                proc.get_atfile(at)?.as_deref(),
                &pathbuf,
                MakeFileSpec::Fifo,
            )?;
        },
    )
}

/// Create a new pipe.
/// `fds[0]` will be written with the pointer to the read end, `fds[1]` the write end.
/// Returns -errno on error, 0 on success.
#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_fs_pipe(fds: *mut [c_int; 2], flags: c_int) -> c_int {
    let proc = process::current().unwrap();
    Errno::extract(
        try {
            let fifos = pipe(flags as u32)?;
            let (fd0, fd1) = proc.insert_dual_file(fifos.0, fifos.1)?;
            let mut fds = UserPtrMut::new_mut(fds)?;
            fds.write([fd0, fd1])?;
        },
    )
}
