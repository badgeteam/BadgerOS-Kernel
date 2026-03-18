// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{
    bindings::{error::Errno, log::LogLevel, raw::timestamp_us_t},
    cpu::thread::{GpRegfile, SpRegfile},
    filesystem::sysimpl::*,
    kernel::sysimpl::*,
    mem::sysimpl::*,
    process::sysimpl::*,
};

pub const SYSCALL_THREAD_YIELD: usize = 0;
pub const SYSCALL_THREAD_SLEEP: usize = 1;
pub const SYSCALL_THREAD_CREATE: usize = 2;
pub const SYSCALL_THREAD_DETACH: usize = 3;
pub const SYSCALL_THREAD_JOIN: usize = 4;
pub const SYSCALL_THREAD_EXIT: usize = 5;
pub const SYSCALL_PROC_EXIT: usize = 6;
pub const SYSCALL_PROC_GETARGS: usize = 7;
pub const SYSCALL_PROC_FORK: usize = 8;
pub const SYSCALL_PROC_EXEC: usize = 9;
pub const SYSCALL_PROC_SIGACTION: usize = 10;
pub const SYSCALL_PROC_SIGRET: usize = 11;
pub const SYSCALL_PROC_WAITPID: usize = 12;
pub const SYSCALL_FS_OPEN: usize = 13;
pub const SYSCALL_FS_CLOSE: usize = 14;
pub const SYSCALL_FS_READ: usize = 15;
pub const SYSCALL_FS_WRITE: usize = 16;
pub const SYSCALL_FS_GETDENTS: usize = 17;
pub const SYSCALL_FS_RENAME: usize = 18;
pub const SYSCALL_FS_STAT: usize = 19;
pub const SYSCALL_FS_MKDIR: usize = 20;
pub const SYSCALL_FS_RMDIR: usize = 21;
pub const SYSCALL_FS_LINK: usize = 22;
pub const SYSCALL_FS_UNLINK: usize = 23;
pub const SYSCALL_FS_MKFIFO: usize = 24;
pub const SYSCALL_FS_PIPE: usize = 25;
pub const SYSCALL_FS_SEEK: usize = 26;
pub const SYSCALL_MEM_MAP: usize = 27;
pub const SYSCALL_MEM_UNMAP: usize = 28;
pub const SYSCALL_MEM_PROTECT: usize = 29;
pub const SYSCALL_TEMP_WRITE: usize = 30;
pub const SYSCALL_SYS_SHUTDOWN: usize = 31;
pub const SYSCALL_TIME_GETTIME: usize = 32;
pub const SYSCALL_THREAD_KILL: usize = 33;
pub const SYSCALL_PROC_KILL: usize = 34;
pub const SYSCALL_PROC_GETID: usize = 35;
pub const SYSCALL_FS_SYMLINK: usize = 36;
pub const SYSCALL_FS_DUP: usize = 37;

pub fn dispatch(regs: &mut GpRegfile, sregs: &mut SpRegfile, args: [usize; 6], sysno: usize) {
    unsafe {
        match sysno {
            SYSCALL_THREAD_YIELD => syscall_thread_yield(),
            SYSCALL_THREAD_SLEEP => {
                regs.set_retval(syscall_thread_sleep(args[0] as timestamp_us_t) as usize)
            }
            SYSCALL_THREAD_CREATE => {
                regs.set_retval(
                    syscall_thread_create(args[0] as _, args[1] as _, args[2] as _) as _,
                )
            }
            SYSCALL_THREAD_DETACH => regs.set_retval(syscall_thread_detach(args[0] as _) as _),
            SYSCALL_THREAD_JOIN => regs.set_retval(syscall_thread_join(args[0] as _) as _),
            SYSCALL_THREAD_EXIT => syscall_thread_exit(args[0] as _),
            SYSCALL_PROC_EXIT => syscall_proc_exit(args[0] as _),
            SYSCALL_PROC_FORK => regs.set_retval(syscall_proc_fork(regs) as _),
            SYSCALL_PROC_EXEC => {
                regs.set_retval(syscall_proc_exec(args[0] as _, args[1] as _, args[2] as _) as _)
            }
            SYSCALL_PROC_SIGACTION => {
                regs.set_retval(
                    syscall_proc_sigaction(args[0] as _, args[1] as _, args[2] as _) as _,
                )
            }
            SYSCALL_PROC_SIGRET => syscall_proc_sigret(regs, sregs),
            SYSCALL_PROC_WAITPID => {
                regs.set_retval(syscall_proc_waitpid(args[0] as _, args[1] as _, args[2] as _) as _)
            }
            SYSCALL_FS_OPEN => {
                regs.set_retval(syscall_fs_open(args[0] as _, args[1] as _, args[2] as _) as _)
            }
            SYSCALL_FS_CLOSE => regs.set_retval(syscall_fs_close(args[0] as _) as _),
            SYSCALL_FS_READ => {
                regs.set_retval(syscall_fs_read(args[0] as _, args[1] as _, args[2] as _) as _)
            }
            SYSCALL_FS_WRITE => {
                regs.set_retval(syscall_fs_write(args[0] as _, args[1] as _, args[2] as _) as _)
            }
            SYSCALL_FS_GETDENTS => {
                regs.set_retval(syscall_fs_getdents(args[0] as _, args[1] as _, args[2] as _) as _)
            }
            SYSCALL_FS_RENAME => regs.set_retval(syscall_fs_rename(
                args[0] as _,
                args[1] as _,
                args[2] as _,
                args[3] as _,
                args[4] as _,
            ) as _),
            SYSCALL_FS_STAT => regs.set_retval(syscall_fs_stat(
                args[0] as _,
                args[1] as _,
                args[2] != 0,
                args[3] as _,
            ) as _),
            SYSCALL_FS_MKDIR => regs.set_retval(syscall_fs_mkdir(args[0] as _, args[1] as _) as _),
            SYSCALL_FS_RMDIR => regs.set_retval(syscall_fs_rmdir(args[0] as _, args[1] as _) as _),
            SYSCALL_FS_LINK => regs.set_retval(syscall_fs_link(
                args[0] as _,
                args[1] as _,
                args[2] as _,
                args[3] as _,
                args[4] as _,
            ) as _),
            SYSCALL_FS_UNLINK => {
                regs.set_retval(syscall_fs_unlink(args[0] as _, args[1] as _) as _)
            }
            SYSCALL_FS_MKFIFO => {
                regs.set_retval(syscall_fs_mkfifo(args[0] as _, args[1] as _) as _)
            }
            SYSCALL_FS_PIPE => regs.set_retval(syscall_fs_pipe(args[0] as _, args[1] as _) as _),
            SYSCALL_FS_SEEK => {
                regs.set_retval(syscall_fs_seek(args[0] as _, args[1] as _, args[2] as _) as _)
            }
            SYSCALL_MEM_MAP => regs.set_retval(syscall_mem_map(
                args[0] as _,
                args[1] as _,
                args[2] as _,
                args[3] as _,
                args[4] as _,
                args[5] as _,
            ) as _),
            SYSCALL_MEM_UNMAP => syscall_mem_unmap(args[0] as _, args[1] as _),
            SYSCALL_MEM_PROTECT => {
                logkf!(LogLevel::Warning, "TODO: protect syscall");
                regs.set_retval(0);
            }
            SYSCALL_TEMP_WRITE => syscall_temp_write(args[0] as _, args[1] as _),
            SYSCALL_TIME_GETTIME => {
                regs.set_retval(syscall_time_gettime(args[0] as _, args[1] as _) as _)
            }
            SYSCALL_THREAD_KILL => {
                regs.set_retval(syscall_thread_kill(args[0] as _, args[1] as _) as _)
            }
            SYSCALL_PROC_KILL => {
                regs.set_retval(syscall_proc_kill(args[0] as _, args[1] as _) as _)
            }
            SYSCALL_PROC_GETID => regs.set_retval(syscall_proc_getid(args[0] as _) as _),
            SYSCALL_FS_SYMLINK => {
                regs.set_retval(syscall_fs_symlink(args[0] as _, args[1] as _, args[2] as _) as _)
            }
            SYSCALL_FS_DUP => {
                regs.set_retval(syscall_fs_dup(args[0] as _, args[1] as _, args[2] as _) as _)
            }
            x => {
                logkf!(LogLevel::Warning, "Unimplemented syscall {}", x);
                regs.set_retval(-(Errno::ENOSYS as i32) as usize);
            }
        }
    }
}
