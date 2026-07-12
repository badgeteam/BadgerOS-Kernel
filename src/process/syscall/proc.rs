// WARNING: This is a generated file, do not edit it!
// SPDX-License-Identifier: CC0

use alloc::{ffi::CString, sync::Arc, vec::Vec};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    cpu::{
        self,
        thread::{GpRegfile, SpRegfile},
    },
    kernel::sched::Thread,
    process::{
        Cmdline, PID, PROCESSES, current,
        signal::signal_die,
        uapi::{
            self,
            signal::{__sa_handler_union, NSIG, SI_USER, SIG_DFL, Signal, sigaction, siginfo_t},
            sigset::sigset_t,
        },
        usercopy::{self, UserPtr, UserPtrMut},
    },
};
use core::{ffi::*, ptr::null};

pub(super) fn exit(code: c_int) -> EResult<()> {
    // W_EXITED.
    let status = (code & 255) << 8;
    current().unwrap().die(status);
    // Nothing needs to be dropped in the scope from which this would be called.
    unsafe { (*Thread::current()).die() };
}

pub(super) fn fork(regs: &mut GpRegfile, _sregs: &mut SpRegfile) -> EResult<PID> {
    let proc = current().unwrap();
    proc.fork(regs).map(|x| x.pid)
}

pub(super) fn exec(path: *const u8, argv: *const *const u8, envp: *const *const u8) -> EResult<()> {
    let proc = current().unwrap();

    let path = usercopy::copy_user_cstr(path)?;

    let mut argbuf = Vec::<CString>::new();
    if argv.is_null() {
        argbuf.try_reserve(1)?;
        argbuf.push(path.clone());
    } else {
        let mut argv = UserPtr::new(argv)?;
        loop {
            let ptr = argv.read()?;
            if ptr.is_null() {
                break;
            }
            argbuf.try_reserve(1)?;
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
            envbuf.try_reserve(1)?;
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

    // TODO: Perhaps a future sched could avoid the need for this.
    unsafe { (*Thread::current()).die() };
}

pub(super) fn sigaction(
    signum: c_int,
    newhandler: Option<UserPtr<sigaction>>,
    oldhandler: Option<UserPtrMut<sigaction>>,
) -> EResult<()> {
    let proc = current().unwrap();
    if signum < 0
        || signum >= NSIG
        || signum == Signal::SIGSTOP as c_int
        || signum == Signal::SIGKILL as c_int
    {
        return Err(Errno::EINVAL);
    }

    let mut guard = proc.sigtab.unintr_lock();
    if let Some(mut oldhandler) = oldhandler {
        oldhandler.write(guard.table[signum as usize])?;
    }
    guard.table[signum as usize] = if let Some(newhandler) = newhandler {
        newhandler.read()?
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

    Ok(())
}

pub(super) fn sigret(regs: &mut GpRegfile, sregs: &mut SpRegfile) -> EResult<()> {
    if unsafe { cpu::usermode::exit_signal(regs, sregs) }.is_err() {
        signal_die(Signal::SIGSEGV as i32);
    }

    logkf!(LogLevel::Debug, "regs:\n{}", regs);
    logkf!(LogLevel::Debug, "sregs:\n{}", sregs);

    Ok(())
}

pub(super) fn waitpid(
    pid: PID,
    wstatus: Option<UserPtrMut<c_int>>,
    options: c_int,
) -> EResult<PID> {
    let proc = current().unwrap();
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
    Ok(res.0)
}

pub(super) fn kill(pid: PID, signum: c_int) -> EResult<()> {
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
    Ok(())
}

pub(super) fn getid(getid_type: c_int) -> EResult<i64> {
    use uapi::getid::*;
    Ok(match getid_type {
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
    })
}
