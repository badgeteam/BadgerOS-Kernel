// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::{c_int, c_long, c_ulong, c_void};

use crate::{
    bindings::{error::Errno, raw::timestamp_us_t},
    kernel::sched::{thread_sleep, thread_yield},
    process::{
        uapi::{
            signal::{SIG_BLOCK, SIG_SETMASK, SIG_UNBLOCK},
            sigset::sigset_t,
        },
        usercopy::UserPtr,
    },
};

use super::sched::Thread;

/// Implementation of thread yield system call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_thread_yield() {
    thread_yield();
}

/// Implementation of usleep system call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_thread_sleep(delay: timestamp_us_t) -> c_int {
    Errno::extract(thread_sleep(delay))
}

/// Create a new thread.
/// Returns thread ID or -errno.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_thread_create(
    entry: *mut c_void,
    arg: *mut c_void,
    priority: c_int,
) -> c_long {
    todo!()
}

/// Detach a thread; the thread will be destroyed as soon as it exits.
/// Returns 0 or -errno.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_thread_detach(u_tid: c_long) -> c_int {
    todo!()
}

/// Wait for a thread to stop and return its exit code.
/// Returns the exit code of that thread or -errno.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_thread_join(u_tid: c_long) -> c_int {
    todo!()
}

/// Exit the current thread; exit code can be read unless destroyed or detached.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_thread_exit(code: c_int) {
    todo!()
}

/// Send a signal to a thread in this process.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_thread_kill(u_tid: c_ulong, signum: c_int) -> c_int {
    todo!()
}

/// Modify the signal mask for this thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_thread_sigmask(
    how: c_int,
    set: *const sigset_t,
    oldset: *mut sigset_t,
) -> c_int {
    let mask = unsafe { &mut (&*Thread::current()).runtime().sigprocmask };
    Errno::extract(
        try {
            if let Some(mut oldset) = UserPtr::new_nullable_mut(oldset)? {
                oldset.write(*mask)?;
            }
            let set = UserPtr::new(set)?.read()?;
            match how {
                SIG_BLOCK => mask.add(&set),
                SIG_UNBLOCK => mask.subtract(&set),
                SIG_SETMASK => *mask = set,
                _ => Err(Errno::EINVAL)?,
            }
        },
    )
}
