use core::{
    cell::UnsafeCell,
    ffi::{c_char, c_int, c_void},
    str::FromStr,
};

use alloc::{boxed::Box, ffi::CString};

use crate::process::Process;

use super::{
    error::EResult,
    raw::{self, SCHED_PRIO_NORMAL, dlist_node_t, dlist_t, tid_t, timestamp_us_t, waitlist_t},
};

pub struct Thread {
    handle: tid_t,
}
unsafe impl Send for Thread {}

unsafe extern "C" fn rust_thread_trampoline(arg: *mut c_void) -> i32 {
    unsafe { Box::from_raw(arg as *mut Box<dyn FnOnce() -> i32>)() }
}

impl Thread {
    pub fn from_id(id: tid_t) -> Self {
        Self { handle: id }
    }
    pub unsafe fn try_new_user(
        process: *const Process,
        user_entrypoint: usize,
        user_arg: usize,
        name: Option<&str>,
    ) -> EResult<Self> {
        let name = name.map(|f| CString::from_str(f).unwrap());
        unsafe {
            let tid = raw::thread_new_user(
                name.map(|f| f.as_ptr()).unwrap_or(0 as *const c_char),
                process as *mut () as *mut raw::process,
                user_entrypoint,
                user_arg,
                SCHED_PRIO_NORMAL as c_int,
            );
            if tid <= 0 {
                Err(core::mem::transmute(-tid))
            } else {
                raw::thread_resume(tid);
                Ok(Self { handle: tid })
            }
        }
    }
    pub fn try_new_kernel<T: FnOnce() -> i32 + Send + 'static>(
        code: T,
        name: Option<&str>,
    ) -> EResult<Self> {
        unsafe {
            let closure: Box<dyn FnOnce() -> i32> = Box::new(code);
            let arg = Box::into_raw(Box::new(closure));
            let name = name.map(|f| CString::from_str(f).unwrap());
            let tid = raw::thread_new_kernel(
                name.map(|f| f.as_ptr()).unwrap_or(0 as *const c_char),
                Some(rust_thread_trampoline),
                arg as *mut c_void,
                SCHED_PRIO_NORMAL as c_int,
            );
            if tid <= 0 {
                drop(Box::from_raw(arg));
                Err(core::mem::transmute(-tid))
            } else {
                raw::thread_resume(tid);
                Ok(Self { handle: tid })
            }
        }
    }
    pub fn new_kernel<T: FnOnce() -> i32 + Send + 'static>(code: T, name: Option<&str>) -> Self {
        Self::try_new_kernel(code, name).unwrap()
    }
    pub fn join(self) -> i32 {
        unsafe { raw::thread_join(self.handle) as i32 }
    }
    pub fn detach(self) {
        // This will drop, which will call the detach function.
    }
    pub fn into_tid(self) -> tid_t {
        let tid = self.handle;
        core::mem::forget(self);
        tid
    }
    pub fn sleep_us(delay: timestamp_us_t) {
        unsafe { raw::thread_sleep(delay) }
    }
    pub unsafe fn exit(code: i32) -> ! {
        unsafe { raw::thread_exit(code) }
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        unsafe { raw::thread_detach(self.handle) };
    }
}

pub struct Waitlist {
    inner: UnsafeCell<waitlist_t>,
}

impl Waitlist {
    /// Create a new waitlist.
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(waitlist_t {
                lock: 1,
                list: dlist_t {
                    len: 0,
                    head: 0 as *mut dlist_node_t,
                    tail: 0 as *mut dlist_node_t,
                },
            }),
        }
    }

    /// Wrapper function that helps with calling the [`FnMut`] for [`Self::block`].
    unsafe extern "C" fn double_check_wrapper(arg: *mut c_void) -> bool {
        let arg = arg as *mut &mut dyn FnMut() -> bool;
        unsafe { (**arg)() }
    }

    /// Block on a waiting list, or until a timeout is reached.
    /// Runs `double_check` and unblocks if false to prevent race conditions causing deadlocks.
    /// Safe to enter interrupts-disabled, but may re-enable them.
    pub fn block(&self, timeout: timestamp_us_t, mut double_check: &mut dyn FnMut() -> bool) {
        unsafe {
            raw::waitlist_block(
                self.inner.as_mut_unchecked(),
                timeout,
                Some(Self::double_check_wrapper),
                &raw mut double_check as *mut c_void,
            )
        }
    }

    /// Resume the first thread from the waiting list, if there is one.
    /// Safe to call with interrupts disabled, which it preserves.
    pub fn notify(&self) {
        unsafe { raw::waitlist_notify(self.inner.as_mut_unchecked()) }
    }
}
