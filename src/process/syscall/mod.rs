
// WARNING: This is a generated file, do not edit it!
// SPDX-License-Identifier: CC0

use crate::{
    bindings::{error::Errno, log::LogLevel},
    cpu::thread::{GpRegfile, SpRegfile},
};
use core::ffi::*;

use super::{
    PID, TID,
    uapi::{
        signal::sigaction, sigset::sigset_t, stat::stat, termios::termios, time::timespec,
        uname::utsname,
    },
    usercopy::{UserPtr, UserPtrMut, UserSlice, UserSliceMut},
};

pub mod thread;
pub mod proc;
pub mod mem;
pub mod time;
pub mod fs;
pub mod sys;

pub static mut SYSCALL_TRACE: bool = false;

pub fn dispatch(regs: &mut GpRegfile, sregs: &mut SpRegfile, args: [usize; 6], sysno: usize) {
    let retval: usize;
    if unsafe { SYSCALL_TRACE } {
        match sysno {
            0 => logkf!(LogLevel::Debug, "syscall thread::yield_time"),
            1 => logkf!(LogLevel::Debug, "syscall thread::sleep"),
            2 => logkf!(LogLevel::Debug, "syscall thread::create"),
            3 => logkf!(LogLevel::Debug, "syscall thread::detach"),
            4 => logkf!(LogLevel::Debug, "syscall thread::join"),
            5 => logkf!(LogLevel::Debug, "syscall thread::exit"),
            6 => logkf!(LogLevel::Debug, "syscall proc::exit"),
            7 => logkf!(LogLevel::Debug, "syscall proc::fork"),
            8 => logkf!(LogLevel::Debug, "syscall proc::exec"),
            9 => logkf!(LogLevel::Debug, "syscall proc::sigaction"),
            10 => logkf!(LogLevel::Debug, "syscall proc::sigret"),
            11 => logkf!(LogLevel::Debug, "syscall proc::waitpid"),
            12 => logkf!(LogLevel::Debug, "syscall fs::open"),
            13 => logkf!(LogLevel::Debug, "syscall fs::close"),
            14 => logkf!(LogLevel::Debug, "syscall fs::read"),
            15 => logkf!(LogLevel::Debug, "syscall fs::write"),
            16 => logkf!(LogLevel::Debug, "syscall fs::getdents"),
            17 => logkf!(LogLevel::Debug, "syscall fs::rename"),
            18 => logkf!(LogLevel::Debug, "syscall fs::stat"),
            19 => logkf!(LogLevel::Debug, "syscall fs::mkdir"),
            20 => logkf!(LogLevel::Debug, "syscall fs::rmdir"),
            21 => logkf!(LogLevel::Debug, "syscall fs::link"),
            22 => logkf!(LogLevel::Debug, "syscall fs::unlink"),
            23 => logkf!(LogLevel::Debug, "syscall fs::mkfifo"),
            24 => logkf!(LogLevel::Debug, "syscall fs::pipe"),
            25 => logkf!(LogLevel::Debug, "syscall fs::seek"),
            26 => logkf!(LogLevel::Debug, "syscall mem::map"),
            27 => logkf!(LogLevel::Debug, "syscall mem::unmap"),
            28 => logkf!(LogLevel::Debug, "syscall mem::protect"),
            29 => logkf!(LogLevel::Debug, "syscall sys::log"),
            30 => logkf!(LogLevel::Debug, "syscall time::gettime"),
            31 => logkf!(LogLevel::Debug, "syscall thread::kill"),
            32 => logkf!(LogLevel::Debug, "syscall proc::kill"),
            33 => logkf!(LogLevel::Debug, "syscall proc::getid"),
            34 => logkf!(LogLevel::Debug, "syscall fs::symlink"),
            35 => logkf!(LogLevel::Debug, "syscall fs::dup"),
            36 => logkf!(LogLevel::Debug, "syscall thread::sigmask"),
            37 => logkf!(LogLevel::Debug, "syscall sys::uname"),
            38 => logkf!(LogLevel::Debug, "syscall fs::isatty"),
            39 => logkf!(LogLevel::Debug, "syscall fs::tcgetattr"),
            40 => logkf!(LogLevel::Debug, "syscall fs::tcsetattr"),
            41 => logkf!(LogLevel::Debug, "syscall fs::getcwd"),
            42 => logkf!(LogLevel::Debug, "syscall fs::chdir"),
            43 => logkf!(LogLevel::Debug, "syscall fs::getfd"),
            44 => logkf!(LogLevel::Debug, "syscall fs::setfd"),
            45 => logkf!(LogLevel::Debug, "syscall fs::getfl"),
            46 => logkf!(LogLevel::Debug, "syscall fs::setfl"),
            x => logkf!(LogLevel::Warning, "unknown syscall {}", x),
        }
    }
    match sysno {
        0 => retval = marshal_thread_yield_time() as _,
        1 => retval = marshal_thread_sleep(args[0] as _) as _,
        2 => retval = marshal_thread_create(args[0] as _, args[1] as _, args[2] as _) as _,
        3 => retval = marshal_thread_detach(args[0] as _) as _,
        4 => retval = marshal_thread_join(args[0] as _) as _,
        5 => retval = marshal_thread_exit(args[0] as _) as _,
        6 => retval = marshal_proc_exit(args[0] as _) as _,
        7 => retval = marshal_proc_fork(regs, sregs) as _,
        8 => retval = marshal_proc_exec(args[0] as _, args[1] as _, args[2] as _) as _,
        9 => retval = marshal_proc_sigaction(args[0] as _, args[1] as _, args[2] as _) as _,
        10 => { marshal_proc_sigret(regs, sregs); return; },
        11 => retval = marshal_proc_waitpid(args[0] as _, args[1] as _, args[2] as _) as _,
        12 => retval = marshal_fs_open(args[0] as _, args[1] as _, args[2] as _) as _,
        13 => retval = marshal_fs_close(args[0] as _) as _,
        14 => retval = marshal_fs_read(args[0] as _, args[1] as _, args[2] as _) as _,
        15 => retval = marshal_fs_write(args[0] as _, args[1] as _, args[2] as _) as _,
        16 => retval = marshal_fs_getdents(args[0] as _, args[1] as _, args[2] as _) as _,
        17 => retval = marshal_fs_rename(args[0] as _, args[1] as _, args[2] as _, args[3] as _, args[4] as _) as _,
        18 => retval = marshal_fs_stat(args[0] as _, args[1] as _, args[2] != 0, args[3] as _) as _,
        19 => retval = marshal_fs_mkdir(args[0] as _, args[1] as _) as _,
        20 => retval = marshal_fs_rmdir(args[0] as _, args[1] as _) as _,
        21 => retval = marshal_fs_link(args[0] as _, args[1] as _, args[2] as _, args[3] as _, args[4] as _) as _,
        22 => retval = marshal_fs_unlink(args[0] as _, args[1] as _) as _,
        23 => retval = marshal_fs_mkfifo(args[0] as _, args[1] as _) as _,
        24 => retval = marshal_fs_pipe(args[0] as _, args[1] as _) as _,
        25 => retval = marshal_fs_seek(args[0] as _, args[1] as _, args[2] as _) as _,
        26 => retval = marshal_mem_map(args[0] as _, args[1] as _, args[2] as _, args[3] as _, args[4] as _, args[5] as _) as _,
        27 => retval = marshal_mem_unmap(args[0] as _, args[1] as _) as _,
        28 => retval = marshal_mem_protect(args[0] as _, args[1] as _, args[2] as _) as _,
        29 => retval = marshal_sys_log(args[0] as _, args[1] as _) as _,
        30 => retval = marshal_time_gettime(args[0] as _, args[1] as _) as _,
        31 => retval = marshal_thread_kill(args[0] as _, args[1] as _) as _,
        32 => retval = marshal_proc_kill(args[0] as _, args[1] as _) as _,
        33 => retval = marshal_proc_getid(args[0] as _) as _,
        34 => retval = marshal_fs_symlink(args[0] as _, args[1] as _, args[2] as _) as _,
        35 => retval = marshal_fs_dup(args[0] as _, args[1] as _, args[2] as _) as _,
        36 => retval = marshal_thread_sigmask(args[0] as _, args[1] as _, args[2] as _) as _,
        37 => retval = marshal_sys_uname(args[0] as _) as _,
        38 => retval = marshal_fs_isatty(args[0] as _) as _,
        39 => retval = marshal_fs_tcgetattr(args[0] as _, args[1] as _) as _,
        40 => retval = marshal_fs_tcsetattr(args[0] as _, args[1] as _) as _,
        41 => retval = marshal_fs_getcwd(args[0] as _, args[1] as _) as _,
        42 => retval = marshal_fs_chdir(args[0] as _, args[1] as _) as _,
        43 => retval = marshal_fs_getfd(args[0] as _) as _,
        44 => retval = marshal_fs_setfd(args[0] as _, args[1] as _) as _,
        45 => retval = marshal_fs_getfl(args[0] as _) as _,
        46 => retval = marshal_fs_setfl(args[0] as _, args[1] as _) as _,
        _ => retval = -(Errno::ENOSYS as i32) as _,
    }
    regs.set_retval(retval);
}

fn marshal_thread_yield_time(
) -> c_int {
    match thread::yield_time(
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_thread_sleep(
    delay: u64,
) -> c_int {
    match thread::sleep(
        delay,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_thread_create(
    entry: usize,
    arg: usize,
    priority: u32,
) -> TID {
    match thread::create(
        entry,
        arg,
        priority,
    ) {
        Ok(x) => x as TID,
        Err(x) => -(x as u32 as TID),
    }
}

fn marshal_thread_detach(
    thread_id: TID,
) -> c_int {
    match thread::detach(
        thread_id,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_thread_join(
    thread_id: TID,
) -> c_int {
    match thread::join(
        thread_id,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_thread_exit(
    code: c_int,
) -> c_int {
    match thread::exit(
        code,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_proc_exit(
    code: c_int,
) -> c_int {
    match proc::exit(
        code,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_proc_fork(
    regs: &mut GpRegfile,
    sregs: &mut SpRegfile,
) -> PID {
    match proc::fork(
        regs,
        sregs,
    ) {
        Ok(x) => x as PID,
        Err(x) => -(x as u32 as PID),
    }
}

fn marshal_proc_exec(
    path: *const u8,
    argv: *const *const u8,
    envp: *const *const u8,
) -> c_int {
    match proc::exec(
        path,
        argv,
        envp,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_proc_sigaction(
    signum: c_int,
    newhandler: *const sigaction,
    oldhandler: *mut sigaction,
) -> c_int {
    let newhandler = match UserPtr::new_nullable(newhandler) {
        Ok(newhandler) => newhandler,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    let oldhandler = match UserPtrMut::new_nullable_mut(oldhandler) {
        Ok(oldhandler) => oldhandler,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match proc::sigaction(
        signum,
        newhandler,
        oldhandler,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_proc_sigret(
    regs: &mut GpRegfile,
    sregs: &mut SpRegfile,
) -> c_int {
    match proc::sigret(
        regs,
        sregs,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_proc_waitpid(
    pid: PID,
    wstatus: *mut c_int,
    options: c_int,
) -> PID {
    let wstatus = match UserPtrMut::new_nullable_mut(wstatus) {
        Ok(wstatus) => wstatus,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match proc::waitpid(
        pid,
        wstatus,
        options,
    ) {
        Ok(x) => x as PID,
        Err(x) => -(x as u32 as PID),
    }
}

fn marshal_fs_open(
    at: c_int,
    path: *const u8,
    oflags: c_int,
) -> c_int {
    match fs::open(
        at,
        path,
        oflags,
    ) {
        Ok(x) => x as c_int,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_close(
    fd: c_int,
) -> c_int {
    match fs::close(
        fd,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_read(
    fd: c_int,
    read_buf: *mut u8,
    read_buf_len: usize
) -> isize {
    let read_buf = match UserSliceMut::new_mut(read_buf, read_buf_len) {
        Ok(read_buf) => read_buf,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match fs::read(
        fd,
        read_buf,
    ) {
        Ok(x) => x as isize,
        Err(x) => -(x as u32 as isize),
    }
}

fn marshal_fs_write(
    fd: c_int,
    write_buf: *const u8,
    write_buf_len: usize
) -> isize {
    let write_buf = match UserSlice::new(write_buf, write_buf_len) {
        Ok(write_buf) => write_buf,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match fs::write(
        fd,
        write_buf,
    ) {
        Ok(x) => x as isize,
        Err(x) => -(x as u32 as isize),
    }
}

fn marshal_fs_getdents(
    fd: c_int,
    read_buf: *mut u8,
    read_buf_len: usize
) -> isize {
    let read_buf = match UserSliceMut::new_mut(read_buf, read_buf_len) {
        Ok(read_buf) => read_buf,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match fs::getdents(
        fd,
        read_buf,
    ) {
        Ok(x) => x as isize,
        Err(x) => -(x as u32 as isize),
    }
}

fn marshal_fs_rename(
    old_at: c_int,
    old_path: *const u8,
    new_at: c_int,
    new_path: *const u8,
    flags: u32,
) -> c_int {
    match fs::rename(
        old_at,
        old_path,
        new_at,
        new_path,
        flags,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_stat(
    fd: c_int,
    path: *const u8,
    follow_link: bool,
    stat_out: *mut stat,
) -> c_int {
    let stat_out = match UserPtrMut::new_mut(stat_out) {
        Ok(stat_out) => stat_out,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match fs::stat(
        fd,
        path,
        follow_link,
        stat_out,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_mkdir(
    at: c_int,
    path: *const u8,
) -> c_int {
    match fs::mkdir(
        at,
        path,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_rmdir(
    at: c_int,
    path: *const u8,
) -> c_int {
    match fs::rmdir(
        at,
        path,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_link(
    old_at: c_int,
    old_path: *const u8,
    new_at: c_int,
    new_path: *const u8,
    flags: u32,
) -> c_int {
    match fs::link(
        old_at,
        old_path,
        new_at,
        new_path,
        flags,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_unlink(
    at: c_int,
    path: *const u8,
) -> c_int {
    match fs::unlink(
        at,
        path,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_mkfifo(
    at: c_int,
    path: *const u8,
) -> c_int {
    match fs::mkfifo(
        at,
        path,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_pipe(
    fds: *mut [c_int; 2],
    flags: c_int,
) -> c_int {
    let fds = match UserPtrMut::new_mut(fds) {
        Ok(fds) => fds,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match fs::pipe(
        fds,
        flags,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_seek(
    fd: c_int,
    offset: i64,
    whence: c_int,
) -> i64 {
    match fs::seek(
        fd,
        offset,
        whence,
    ) {
        Ok(x) => x as i64,
        Err(x) => -(x as u32 as i64),
    }
}

fn marshal_mem_map(
    address: usize,
    size: usize,
    prot: c_int,
    flags: c_int,
    fd: c_int,
    offset: i64,
) -> isize {
    match mem::map(
        address,
        size,
        prot,
        flags,
        fd,
        offset,
    ) {
        Ok(x) => x as isize,
        Err(x) => -(x as u32 as isize),
    }
}

fn marshal_mem_unmap(
    address: usize,
    size: usize,
) -> c_int {
    match mem::unmap(
        address,
        size,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_mem_protect(
    address: usize,
    size: usize,
    prot: c_int,
) -> c_int {
    match mem::protect(
        address,
        size,
        prot,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_sys_log(
    message: *const u8,
    message_len: usize
) -> c_int {
    let message = match UserSlice::new(message, message_len) {
        Ok(message) => message,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match sys::log(
        message,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_time_gettime(
    clkid: c_int,
    time: *mut timespec,
) -> c_int {
    let time = match UserPtrMut::new_mut(time) {
        Ok(time) => time,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match time::gettime(
        clkid,
        time,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_thread_kill(
    thread_id: TID,
    signum: c_int,
) -> c_int {
    match thread::kill(
        thread_id,
        signum,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_proc_kill(
    pid: PID,
    signum: c_int,
) -> c_int {
    match proc::kill(
        pid,
        signum,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_proc_getid(
    getid_type: c_int,
) -> i64 {
    match proc::getid(
        getid_type,
    ) {
        Ok(x) => x as i64,
        Err(x) => -(x as u32 as i64),
    }
}

fn marshal_fs_symlink(
    link_target: *const u8,
    at: c_int,
    path: *const u8,
) -> c_int {
    match fs::symlink(
        link_target,
        at,
        path,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_dup(
    fd: c_int,
    flags: c_int,
    newfd: c_int,
) -> c_int {
    match fs::dup(
        fd,
        flags,
        newfd,
    ) {
        Ok(x) => x as c_int,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_thread_sigmask(
    how: c_int,
    set: *const sigset_t,
    oldset: *mut sigset_t,
) -> c_int {
    let set = match UserPtr::new_nullable(set) {
        Ok(set) => set,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    let oldset = match UserPtrMut::new_nullable_mut(oldset) {
        Ok(oldset) => oldset,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match thread::sigmask(
        how,
        set,
        oldset,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_sys_uname(
    name: *mut utsname,
) -> c_int {
    let name = match UserPtrMut::new_mut(name) {
        Ok(name) => name,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match sys::uname(
        name,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_isatty(
    fd: c_int,
) -> c_int {
    match fs::isatty(
        fd,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_tcgetattr(
    fd: c_int,
    attr: *mut termios,
) -> c_int {
    let attr = match UserPtrMut::new_mut(attr) {
        Ok(attr) => attr,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match fs::tcgetattr(
        fd,
        attr,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_tcsetattr(
    fd: c_int,
    attr: *const termios,
) -> c_int {
    let attr = match UserPtr::new(attr) {
        Ok(attr) => attr,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match fs::tcsetattr(
        fd,
        attr,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_getcwd(
    buf: *mut u8,
    buf_len: usize
) -> c_int {
    let buf = match UserSliceMut::new_mut(buf, buf_len) {
        Ok(buf) => buf,
        Err(_) => return -(Errno::EFAULT as i32) as _,
    };
    match fs::getcwd(
        buf,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_chdir(
    at: c_int,
    path: *const u8,
) -> c_int {
    match fs::chdir(
        at,
        path,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_getfd(
    fd: c_int,
) -> c_int {
    match fs::getfd(
        fd,
    ) {
        Ok(x) => x as c_int,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_setfd(
    fd: c_int,
    flags: c_int,
) -> c_int {
    match fs::setfd(
        fd,
        flags,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_getfl(
    fd: c_int,
) -> c_int {
    match fs::getfl(
        fd,
    ) {
        Ok(x) => x as c_int,
        Err(x) => -(x as u32 as c_int),
    }
}

fn marshal_fs_setfl(
    fd: c_int,
    flags: c_int,
) -> c_int {
    match fs::setfl(
        fd,
        flags,
    ) {
        Ok(()) => 0,
        Err(x) => -(x as u32 as c_int),
    }
}
