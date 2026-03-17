// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ffi::{c_char, c_int},
    ptr::null,
};

use alloc::{ffi::CString, sync::Arc, vec::Vec};

use crate::{
    badgelib::time::Timespec,
    bindings::{
        error::{EResult, Errno},
        raw::rawputc,
    },
    cpu::{
        self,
        thread::{GpRegfile, SpRegfile},
    },
    kernel::sched::Thread,
    process::{signal::signal_die, usercopy},
};

use super::{
    Cmdline, PID, PROCESSES, current,
    uapi::{
        self,
        inttypes::pid_t,
        signal::{__sa_handler_union, NSIG, SI_USER, SIG_DFL, Signal, sigaction, siginfo_t},
        sigset::sigset_t,
        time::timespec,
    },
    usercopy::{UserPtr, UserPtrMut, UserSlice},
};

#[unsafe(no_mangle)]
#[inline(never)]
pub unsafe extern "C" fn syscall_temp_write(message: *const c_char, length: usize) {
    let _: EResult<()> = try {
        let slice = UserSlice::new(message as *mut c_char, length)?;
        for i in 0..slice.len() {
            unsafe { rawputc(slice.read(i)?) };
        }
    };
}

/// Exit the process; exit code can be read by parent process.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_exit(code: c_int) {
    // W_EXITED.
    let status = (code & 255) << 8;
    current().unwrap().die(status);
    // Nothing needs to be dropped in the scope from which this would be called.
    unsafe { (*Thread::current()).die() };
}

/// Create a copy of the running process and return its PID (to the parent) or -1 (to the child).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_fork(regs: &GpRegfile) -> pid_t {
    let proc = current().unwrap();
    Errno::extract_i64(try { proc.fork(regs)?.pid })
}

/// Execute the program at `path`, replacing the calling program's code and data in the process.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_exec(
    path: *const c_char,
    argv: *const *const c_char,
    envp: *const *const c_char,
) -> c_int {
    let res = Errno::extract(
        try {
            let proc = current().unwrap();

            let path = usercopy::copy_user_cstr(path)?;

            let mut argbuf = Vec::<CString>::new();
            if argv.is_null() {
                argbuf.try_reserve(1).map_err(Into::into)?;
                argbuf.push(path.clone());
            } else {
                let mut argv = UserPtr::new(argv)?;
                loop {
                    let ptr = argv.read()?;
                    if ptr.is_null() {
                        break;
                    }
                    argbuf.try_reserve(1).map_err(Into::into)?;
                    argbuf.push(usercopy::copy_user_cstr(ptr)?);
                    argv = UserPtr::new(argv.as_ptr().wrapping_add(1))?;
                }
            }

            let mut envbuf = Vec::<CString>::new();
            if envp.is_null() {
                envbuf = proc.cmdline().envp.clone();
            } else {
                let mut envp = UserPtr::new(envp)?;
                loop {
                    let ptr = envp.read()?;
                    if ptr.is_null() {
                        break;
                    }
                    envbuf.try_reserve(1).map_err(Into::into)?;
                    envbuf.push(usercopy::copy_user_cstr(ptr)?);
                    envp = UserPtr::new(envp.as_ptr().wrapping_add(1))?;
                }
            }

            proc.exec(Cmdline {
                binary: path,
                argv: argbuf,
                envp: envbuf,
                auxv: Vec::new(),
            })?;
        },
    );

    if res == 0 {
        // TODO: Perhaps a future sched could avoid the need for this.
        unsafe { (*Thread::current()).die() };
    }

    res
}

/// Set the signal handler for a specific signal number.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_sigaction(
    signum: c_int,
    act: *const sigaction,
    old_act: *mut sigaction,
) -> c_int {
    let proc = current().unwrap();
    if signum < 0
        || signum >= NSIG
        || signum == Signal::SIGSTOP as c_int
        || signum == Signal::SIGKILL as c_int
    {
        return -(Errno::EINVAL as c_int);
    }
    Errno::extract(
        try {
            let act = UserPtr::new_nullable(act)?;
            let old_act = UserPtr::new_nullable_mut(old_act)?;
            let mut guard = proc.sigtab.unintr_lock();
            if let Some(mut old_act) = old_act {
                old_act.write(guard.table[signum as usize])?;
            }
            guard.table[signum as usize] = if let Some(act) = act {
                act.read()?
            } else {
                sigaction {
                    __sa_handler: __sa_handler_union {
                        sa_handler: SIG_DFL as *const fn(i32),
                    },
                    sa_flags: 0,
                    sa_restorer: null(),
                    sa_mask: sigset_t::default(),
                }
            };
        },
    )
}

/// Return from a signal handler.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_sigret(regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    if unsafe { cpu::usermode::exit_signal(regs, sregs) }.is_err() {
        signal_die(uapi::signal::Signal::SIGSEGV as i32);
    }
}

/// Get child process status update.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_waitpid(
    pid: PID,
    wstatus: *mut c_int,
    options: c_int,
) -> pid_t {
    let proc = current().unwrap();
    Errno::extract_u64(
        try {
            let wstatus = UserPtr::new_nullable_mut(wstatus)?;
            let res = if pid < -1 {
                Err(Errno::ENOSYS)?;
                (pid, 0) // TODO: process groups
            } else if pid > 0 {
                // Find target process.
                let child = PROCESSES
                    .lock_shared()?
                    .get(&pid)
                    .cloned()
                    .ok_or(Errno::ECHILD)?;

                // Enforce that it is a child.
                if !Arc::ptr_eq(
                    &proc,
                    &child
                        .pcr
                        .lock_shared()?
                        .parent
                        .upgrade()
                        .ok_or(Errno::ECHILD)?,
                ) {
                    Err(Errno::ECHILD)?;
                }

                (pid, child.wait(options)?)
            } else {
                proc.wait_children(options)?
            };
            if let Some(mut wstatus) = wstatus {
                wstatus.write(res.1)?;
            }
            res.0 as u64
        },
    )
}

/// Get the value of some clock.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_time_gettime(_clkid: c_int, timespec: *mut timespec) -> c_int {
    Errno::extract(
        try {
            let mut timespec = UserPtrMut::new_mut(timespec)?;
            timespec.write(Timespec::now().into())?;
        },
    )
}

/// Send a signal to an arbitrary thread in a specified process. See man kill(2).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_kill(pid: pid_t, signum: c_int) -> c_int {
    Errno::extract(
        try {
            if signum > 1023 {
                Err(Errno::EPERM)?;
            }
            if pid < 1 {
                Err(Errno::ESRCH)?;
            }
            let proc = PROCESSES
                .lock_shared()?
                .get(&pid)
                .ok_or(Errno::ESRCH)?
                .clone();
            proc.send_async_sig(siginfo_t {
                si_signo: signum,
                si_code: SI_USER,
                si_errno: 0,
                __si_fields: Default::default(),
            });
        },
    )
}

/// Get an ID as specified by _GETID_* macros.
pub unsafe extern "C" fn syscall_proc_getid(getid_type: c_int) -> i64 {
    use uapi::getid::*;
    match getid_type {
        GETID_PID => current().unwrap().pid,
        GETID_PPID => current()
            .unwrap()
            .pcr
            .unintr_lock_shared()
            .parent
            .upgrade()
            .map(|x| x.pid)
            .unwrap_or(0),
        GETID_TID => 0,
        GETID_UID => 0,
        GETID_EUID => 0,
        GETID_GID => 0,
        GETID_EGID => 0,
        _ => 0,
    }
}
