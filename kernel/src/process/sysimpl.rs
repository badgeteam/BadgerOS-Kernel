// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ffi::{c_char, c_int, c_void},
    ptr::null_mut,
};

use alloc::{ffi::CString, vec::Vec};

use crate::{
    bindings::{
        error::{EResult, Errno},
        raw::{
            SIGKILL, SIGSEGV, SIGSTOP, rawputc, sched_signal_exit, sigaction,
            sigaction__bindgen_ty_1, siginfo_t, thread_exit,
        },
    },
    filesystem::PATH_MAX,
    process::usercopy,
};

use super::{
    Cmdline, PID,
    c_api::get_user_pc,
    current,
    signal::{SIG_COUNT, SIG_DFL, run_handler},
    usercopy::{UserPtr, UserSlice},
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
    unsafe { thread_exit(0) };
}

/// Get the command-line arguments (i.e. argc+argv) of the current process.
/// If memory is large enough, a NULL-terminated argv array of C-string poc_inters and their data is stored in `memory`.
/// The function returns how many bytes would be needed to store the structure.
/// If the memory was not large enough, it it not modified.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_getargs(cap: usize, memory: *mut c_void) -> usize {
    todo!()
}

/// Create a copy of the running process and return its PID (to the parent) or -1 (to the child).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_fork() -> i64 {
    let proc = current().unwrap();
    Errno::extract_i64(try { proc.fork()?.pid })
}

/// Execute the program at `path`, replacing the calling program's code and data in the process.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_exec(
    path: *const c_char,
    argv: *const *const c_char,
    envp: *const *const c_char,
) -> c_int {
    // TODO: Implement envp.
    let res = Errno::extract(
        try {
            let proc = current().unwrap();

            let path = usercopy::copy_user_cstr(path)?;

            let mut argv = UserPtr::new(argv)?;
            let mut argbuf = Vec::<CString>::new();
            loop {
                let ptr = argv.read()?;
                if ptr.is_null() {
                    break;
                }
                argbuf.try_reserve(1).map_err(Into::into)?;
                argbuf.push(usercopy::copy_user_cstr(ptr)?);
                argv = UserPtr::new(argv.as_ptr().wrapping_add(1))?;
            }

            proc.exec(Cmdline {
                binary: path,
                argv: argbuf,
            })?;
        },
    );

    if res == 0 {
        // TODO: Perhaps a future sched could avoid the need for this.
        unsafe { thread_exit(0) };
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
    if signum < 0 || signum >= SIG_COUNT || signum == SIGSTOP as c_int || signum == SIGKILL as c_int
    {
        return -(Errno::EINVAL as c_int);
    }
    Errno::extract(
        try {
            let act = UserPtr::new_nullable(act)?;
            let old_act = UserPtr::new_nullable_mut(old_act)?;
            let mut guard = proc.sigtab.lock();
            if let Some(mut old_act) = old_act {
                old_act.write(guard.table[signum as usize])?;
            }
            guard.table[signum as usize] = if let Some(act) = act {
                act.read()?
            } else {
                sigaction {
                    __bindgen_anon_1: sigaction__bindgen_ty_1 {
                        sa_handler_ptr: SIG_DFL as *mut c_void,
                    },
                    sa_mask: 0,
                    sa_flags: 0,
                    sa_return_trampoline: null_mut(),
                }
            };
        },
    )
}

/// Return from a signal handler.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_proc_sigret() {
    if !unsafe { sched_signal_exit() } {
        run_handler(siginfo_t {
            si_signo: SIGSEGV as c_int,
            si_code: 0,
            si_pid: current().unwrap().pid,
            si_uid: 0,
            si_addr: get_user_pc() as *mut c_void,
            si_status: 0,
        });
    }
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
