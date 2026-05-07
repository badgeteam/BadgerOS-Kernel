// WARNING: This is a generated file, do not edit it!
// SPDX-License-Identifier: CC0

use crate::{
    bindings::{
        error::{EResult, Errno},
        raw::timestamp_us_t,
    },
    kernel::sched::{Thread, thread_sleep, thread_yield},
    process::{
        TID,
        uapi::{
            signal::{SIG_BLOCK, SIG_SETMASK, SIG_UNBLOCK},
            sigset::sigset_t,
        },
        usercopy::{UserPtr, UserPtrMut},
    },
};
use core::ffi::*;

pub(super) fn yield_time() -> EResult<()> {
    thread_yield();
    Ok(())
}

pub(super) fn sleep(delay: u64) -> EResult<()> {
    thread_sleep(delay.try_into().unwrap_or(timestamp_us_t::MAX))
}

pub(super) fn create(_entry: usize, _arg: usize, _priority: u32) -> EResult<TID> {
    Err(Errno::ENOSYS)
}

pub(super) fn detach(_thread_id: TID) -> EResult<()> {
    Err(Errno::ENOSYS)
}

pub(super) fn join(_thread_id: TID) -> EResult<()> {
    Err(Errno::ENOSYS)
}

pub(super) fn exit(_code: c_int) -> EResult<()> {
    Err(Errno::ENOSYS)
}

pub(super) fn kill(_thread_id: TID, _signum: c_int) -> EResult<()> {
    Err(Errno::ENOSYS)
}

pub(super) fn sigmask(
    how: c_int,
    set: Option<UserPtr<sigset_t>>,
    oldset: Option<UserPtrMut<sigset_t>>,
) -> EResult<()> {
    let mask = unsafe { &mut (&*Thread::current()).runtime().sigprocmask };

    if let Some(mut oldset) = oldset {
        oldset.write(*mask)?;
    }

    if let Some(set) = set {
        let set = set.read()?;
        match how {
            SIG_BLOCK => mask.add(&set),
            SIG_UNBLOCK => mask.subtract(&set),
            SIG_SETMASK => *mask = set,
            _ => return Err(Errno::EINVAL),
        }
    }

    Ok(())
}
