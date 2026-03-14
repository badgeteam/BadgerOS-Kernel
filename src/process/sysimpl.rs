// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ffi::{c_char, c_int},
    ptr::null,
};

use alloc::{ffi::CString, vec::Vec};

use crate::{
    badgelib::time::Timespec,
    bindings::{
        error::{EResult, Errno},
        raw::rawputc,
    },
    cpu::thread::GpRegfile,
    kernel::sched::Thread,
    process::{signal::signal_die, usercopy},
};

use super::{
    Cmdline, PID, current,
    uapi::{
        self,
        signal::{__sa_handler_union, NSIG, SIG_DFL, Signal, sigaction},
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
    current().unwrap().kill(status);
    // Nothing needs to be dropped in the scope from which this would be called.
    unsafe { (*Thread::current()).die() };
}

/// Create a copy of the running process and return its PID (to the parent) or -1 (to the child).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_fork(regs: &GpRegfile) -> i64 {
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
                    sa_mask: sigset_t { __sig: [0; _] },
                }
            };
        },
    )
}

/// Return from a signal handler.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_sigret() {
    // if !unsafe { sched_signal_exit() } {
    // TODO.
    signal_die(uapi::signal::Signal::SIGSEGV as i32);
    // run_handler(siginfo_t {
    //     si_signo: SIGSEGV as c_int,
    //     si_code: 0,
    //     si_pid: current().unwrap().pid,
    //     si_uid: 0,
    //     si_addr: get_user_pc() as *mut c_void,
    //     si_status: 0,
    // });
    // }
}

/// Get child process status update.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_waitpid(
    pid: PID,
    wstatus: *mut c_int,
    options: c_int,
) -> c_int {
    todo!()
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
