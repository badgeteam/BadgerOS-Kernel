// WARNING: This is a generated file, do not edit it!
// SPDX-License-Identifier: CC0

use bytemuck::bytes_of;

use crate::{
    bindings::{
        device::HasBaseDevice,
        error::{EResult, Errno},
        raw::{seek_mode_t_SEEK_CUR, seek_mode_t_SEEK_END, seek_mode_t_SEEK_SET},
    },
    filesystem::{self, Dirent, MakeFileSpec, NodeType, PATH_MAX, SeekMode},
    process::{
        self, FILE_MAX,
        files::FileDesc,
        uapi::{dirent, stat::stat, termios::termios},
        usercopy::{self, AccessResult, UserPtr, UserPtrMut, UserSlice, UserSliceMut},
    },
};
use core::{
    ffi::*,
    sync::atomic::{AtomicU32, Ordering},
};

/// Helper type for [`syscall_fs_getdents`].
pub struct DentBuffer<'a> {
    index: usize,
    slice: UserSliceMut<'a, u8>,
}

impl<'a> DentBuffer<'a> {
    pub fn new(slice: UserSliceMut<'a, u8>) -> Self {
        Self { slice, index: 0 }
    }

    pub fn push(&mut self, dent: Dirent) -> AccessResult<bool> {
        let mut d_reclen =
            (size_of::<dirent::dirent_headeronly>() + dent.name.len() + 1) as c_ushort;
        if d_reclen % 8 != 0 {
            d_reclen += 8 - d_reclen % 8;
        }
        let header = dirent::dirent_headeronly {
            d_ino: dent.ino,
            d_off: dent.dirent_off as i64,
            d_reclen,
            d_type: match dent.type_ {
                NodeType::Unknown => dirent::DT_UNKNOWN,
                NodeType::Fifo => dirent::DT_FIFO,
                NodeType::CharDev => dirent::DT_CHR,
                NodeType::Directory => dirent::DT_DIR,
                NodeType::BlockDev => dirent::DT_CHR,
                NodeType::Regular => dirent::DT_REG,
                NodeType::Symlink => dirent::DT_LNK,
                NodeType::UnixSocket => dirent::DT_SOCK,
            },
        };

        if self.index + d_reclen as usize > self.slice.len() {
            return Ok(false);
        }

        self.slice.write_multiple(self.index, bytes_of(&header))?;
        self.slice.write_multiple(
            self.index + size_of::<dirent::dirent_headeronly>(),
            &dent.name,
        )?;
        self.slice.write(
            self.index + size_of::<dirent::dirent_headeronly>() + dent.name.len(),
            0,
        )?;
        self.index += d_reclen as usize;

        Ok(true)
    }
}

pub(super) fn open(at: c_int, path: *const u8, oflags: c_int) -> EResult<c_int> {
    let proc = process::current().unwrap();
    let mut files = proc.files.lock()?;
    let mut pathbuf = [0u8; PATH_MAX];
    let pathlen = usercopy::read_user_cstr(path, &mut pathbuf)?;
    let at_file = files.get_atfile(at)?;
    let file = filesystem::open(
        at_file.as_deref(),
        &pathbuf[..pathlen],
        oflags as u32 & 0xffff,
    )?;
    files.insert_file(
        0,
        FileDesc {
            flags: AtomicU32::new(oflags as u32 & 0xffff0000),
            file,
        },
    )
}

pub(super) fn close(fd: c_int) -> EResult<()> {
    let proc = process::current().unwrap();
    proc.files.unintr_lock().remove_file(fd as i32)
}

pub(super) fn read(fd: c_int, read_buf: UserSliceMut<u8>) -> EResult<usize> {
    let proc = process::current().unwrap();
    proc.files.lock_shared()?.get_file(fd)?.read(read_buf)
}

pub(super) fn write(fd: c_int, write_buf: UserSlice<u8>) -> EResult<usize> {
    let proc = process::current().unwrap();
    proc.files.lock_shared()?.get_file(fd)?.write(write_buf)
}

pub(super) fn getdents(fd: c_int, read_buf: UserSliceMut<u8>) -> EResult<usize> {
    let proc = process::current().unwrap();
    let mut buffer = DentBuffer::new(read_buf);
    proc.files
        .lock_shared()?
        .get_file(fd)?
        .get_dirents(&mut buffer)?;
    Ok(buffer.index)
}

pub(super) fn rename(
    old_at: c_int,
    old_path: *const u8,
    new_at: c_int,
    new_path: *const u8,
    flags: u32,
) -> EResult<()> {
    let proc = process::current().unwrap();
    let files = proc.files.lock_shared()?;
    let old_at_file = files.get_atfile(old_at)?;
    let mut old_pathbuf = [0u8; PATH_MAX];
    let old_pathlen = usercopy::read_user_cstr(old_path, &mut old_pathbuf)?;
    let new_at_file = files.get_atfile(new_at)?;
    let mut new_pathbuf = [0u8; PATH_MAX];
    let new_pathlen = usercopy::read_user_cstr(new_path, &mut new_pathbuf)?;
    filesystem::rename(
        old_at_file.as_deref(),
        &old_pathbuf[..old_pathlen],
        new_at_file.as_deref(),
        &new_pathbuf[..new_pathlen],
        flags,
    )
}

pub(super) fn stat(
    fd: c_int,
    path: *const u8,
    follow_link: bool,
    mut stat_out: UserPtrMut<stat>,
) -> EResult<()> {
    let proc = process::current().unwrap();
    let files = proc.files.lock_shared()?;
    let stat: stat;
    if path.is_null() {
        stat = files.get_file(fd)?.stat()?.into();
    } else {
        let mut pathbuf = [0u8; PATH_MAX];
        let pathlen = usercopy::read_user_cstr(path, &mut pathbuf)?;
        stat = filesystem::open(
            files.get_atfile(fd)?.as_deref(),
            &pathbuf[..pathlen],
            if follow_link {
                0
            } else {
                filesystem::oflags::NOFOLLOW
            },
        )?
        .stat()?
        .into();
    }
    stat_out.write(stat)
}

pub(super) fn mkdir(at: c_int, path: *const u8) -> EResult<()> {
    let proc = process::current().unwrap();
    let mut pathbuf = [0u8; PATH_MAX];
    let pathlen = usercopy::read_user_cstr(path, &mut pathbuf)?;
    filesystem::make_file(
        proc.files.lock_shared()?.get_atfile(at)?.as_deref(),
        &pathbuf[..pathlen],
        MakeFileSpec::Directory,
    )
}

pub(super) fn rmdir(at: c_int, path: *const u8) -> EResult<()> {
    let proc = process::current().unwrap();
    let mut pathbuf = [0u8; PATH_MAX];
    let pathlen = usercopy::read_user_cstr(path, &mut pathbuf)?;
    filesystem::unlink(
        proc.files.lock_shared()?.get_atfile(at)?.as_deref(),
        &pathbuf[..pathlen],
        true,
    )
}

pub(super) fn link(
    old_at: c_int,
    old_path: *const u8,
    new_at: c_int,
    new_path: *const u8,
    flags: u32,
) -> EResult<()> {
    let proc = process::current().unwrap();
    let files = proc.files.lock_shared()?;
    let old_at_file = files.get_atfile(old_at)?;
    let mut old_pathbuf = [0u8; PATH_MAX];
    let old_pathlen = usercopy::read_user_cstr(old_path, &mut old_pathbuf)?;
    let new_at_file = files.get_atfile(new_at)?;
    let mut new_pathbuf = [0u8; PATH_MAX];
    let new_pathlen = usercopy::read_user_cstr(new_path, &mut new_pathbuf)?;
    filesystem::link(
        old_at_file.as_deref(),
        &old_pathbuf[..old_pathlen],
        new_at_file.as_deref(),
        &new_pathbuf[..new_pathlen],
        flags,
    )
}

pub(super) fn unlink(at: c_int, path: *const u8) -> EResult<()> {
    let proc = process::current().unwrap();
    let mut pathbuf = [0u8; PATH_MAX];
    let pathlen = usercopy::read_user_cstr(path, &mut pathbuf)?;
    filesystem::unlink(
        proc.files.lock_shared()?.get_atfile(at)?.as_deref(),
        &pathbuf[..pathlen],
        false,
    )
}

pub(super) fn mkfifo(at: c_int, path: *const u8) -> EResult<()> {
    let proc = process::current().unwrap();
    let mut pathbuf = [0u8; PATH_MAX];
    let pathlen = usercopy::read_user_cstr(path, &mut pathbuf)?;
    filesystem::make_file(
        proc.files.lock_shared()?.get_atfile(at)?.as_deref(),
        &pathbuf[..pathlen],
        MakeFileSpec::Fifo,
    )
}

pub(super) fn pipe(mut fds: UserPtrMut<[c_int; 2]>, flags: c_int) -> EResult<()> {
    let proc = process::current().unwrap();
    let fifos = filesystem::pipe(flags as u32)?;
    let (fd0, fd1) = proc.files.unintr_lock().insert_dual_file(
        FileDesc {
            flags: AtomicU32::new(flags as u32 & 0xffff0000),
            file: fifos.0,
        },
        FileDesc {
            flags: AtomicU32::new(flags as u32 & 0xffff0000),
            file: fifos.1,
        },
    )?;
    fds.write([fd0, fd1])?;
    Ok(())
}

pub(super) fn seek(fd: c_int, offset: i64, whence: c_int) -> EResult<u64> {
    #[allow(non_upper_case_globals)]
    let mode = match whence as u32 {
        seek_mode_t_SEEK_CUR => SeekMode::Cur,
        seek_mode_t_SEEK_SET => SeekMode::Set,
        seek_mode_t_SEEK_END => SeekMode::End,
        _ => return Err(Errno::EINVAL),
    };
    let proc = process::current().unwrap();
    proc.files.lock_shared()?.get_file(fd)?.seek(mode, offset)
}

pub(super) fn symlink(link_target: *const u8, at: c_int, path: *const u8) -> EResult<()> {
    let mut targetbuf = [0u8; PATH_MAX];
    let targetlen = usercopy::read_user_cstr(link_target, &mut targetbuf)?;
    let proc = process::current().unwrap();
    let mut pathbuf = [0u8; PATH_MAX];
    let pathlen = usercopy::read_user_cstr(path, &mut pathbuf)?;
    filesystem::make_file(
        proc.files.lock_shared()?.get_atfile(at)?.as_deref(),
        &pathbuf[..pathlen],
        MakeFileSpec::Symlink(&targetbuf[..targetlen]),
    )
}

pub(super) fn dup(fd: c_int, flags: c_int, newfd: c_int) -> EResult<c_int> {
    let proc = process::current().unwrap();
    let mut files = proc.files.lock()?;
    let fd = files.get_file(fd)?;
    if newfd == -1 {
        files.insert_file(
            0,
            FileDesc {
                flags: AtomicU32::new(flags as u32 & 0xffff_0000),
                file: fd,
            },
        )
    } else if flags as u32 & filesystem::oflags::DUP_FCNTL != 0 {
        files.insert_file(
            newfd,
            FileDesc {
                flags: AtomicU32::new(flags as u32 & 0xffff_0000),
                file: fd,
            },
        )
    } else if newfd > 0 && newfd < FILE_MAX {
        files.replace_file(
            newfd,
            FileDesc {
                flags: AtomicU32::new(flags as u32 & 0xffff_0000),
                file: fd,
            },
        )?;
        Ok(newfd)
    } else {
        Err(Errno::EINVAL)
    }
}

pub(super) fn isatty(fd: c_int) -> EResult<()> {
    let proc = process::current().unwrap();
    proc.files.lock_shared()?.get_file(fd)?.isatty()
}

pub(super) fn tcgetattr(fd: c_int, mut attr: UserPtrMut<termios>) -> EResult<()> {
    let proc = process::current().unwrap();
    let files = proc.files.lock_shared()?;
    let fd = files.get_file(fd)?;
    let mut buf = Default::default();
    fd.get_device()
        .ok_or(Errno::ENOTTY)?
        .as_tty()
        .ok_or(Errno::ENOTTY)?
        .getattr(&mut buf)?;
    attr.write(buf)
}

pub(super) fn tcsetattr(fd: c_int, attr: UserPtr<termios>) -> EResult<()> {
    let proc = process::current().unwrap();
    let files = proc.files.lock_shared()?;
    let fd = files.get_file(fd)?;
    let buf = attr.read()?;
    fd.get_device()
        .ok_or(Errno::ENOTTY)?
        .as_tty()
        .ok_or(Errno::ENOTTY)?
        .setattr(&buf)
}

pub(super) fn getcwd(mut buf: UserSliceMut<u8>) -> EResult<()> {
    let proc = process::current().unwrap();
    let files = proc.files.lock_shared()?;
    let cwd: &[u8] = &files.cwd;
    if buf.len() < cwd.len() + 1 {
        Err(Errno::ERANGE)?;
    }
    buf.write_multiple(0, cwd)?;
    buf.write(cwd.len(), 0)?;
    Ok(())
}

pub(super) fn chdir(at: c_int, path: *const u8) -> EResult<()> {
    let proc = process::current().unwrap();

    if at >= 0 && !path.is_null() {
        return Err(Errno::EINVAL);
    }

    let mut files = proc.files.lock()?;
    if at >= 0 {
        let file = files.get_file(at)?;
        files.fchdir(file)?;
    } else {
        let mut pathbuf = [0u8; PATH_MAX];
        let pathlen = usercopy::read_user_cstr(path, &mut pathbuf)?;
        files.chdir(&pathbuf[..pathlen])?;
    }

    Ok(())
}

pub(super) fn getfd(fd: c_int) -> EResult<c_int> {
    let proc = process::current().unwrap();
    Ok(proc
        .files
        .lock_shared()?
        .inner
        .get(&fd)
        .ok_or(Errno::EBADF)?
        .flags
        .load(Ordering::Relaxed) as i32)
}

pub(super) fn setfd(fd: c_int, flags: c_int) -> EResult<()> {
    let proc = process::current().unwrap();
    proc.files
        .lock_shared()?
        .inner
        .get(&fd)
        .ok_or(Errno::EBADF)?
        .flags
        .store(flags as u32, Ordering::Relaxed);
    Ok(())
}

pub(super) fn getfl(fd: c_int) -> EResult<c_int> {
    let proc = process::current().unwrap();

    Ok(proc.files.lock_shared()?.get_file(fd)?.get_flags() as c_int)
}

pub(super) fn setfl(fd: c_int, flags: c_int) -> EResult<()> {
    let proc = process::current().unwrap();

    proc.files
        .lock_shared()?
        .get_file(fd)?
        .set_flags(flags as u32)?;

    Ok(())
}
