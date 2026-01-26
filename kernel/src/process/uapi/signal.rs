// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#![allow(non_camel_case_types)]

use crate::process::usercopy::UserCopyable;
use core::ffi::{c_int, c_ulong, c_void};

use super::sigset::sigset_t;
pub type __sighandler = *const fn(c_int);

pub const SIG_ERR: *mut c_void = usize::MAX as *mut c_void;
pub const SIG_DFL: *mut c_void = 0 as *mut c_void;
pub const SIG_IGN: *mut c_void = 1 as *mut c_void;

pub mod siginfo {
    use super::super::inttypes::{clock_t, pid_t, uid_t};
    use crate::process::{uapi::sigval::sigval, usercopy::UserCopyable};
    use core::ffi::{c_char, c_int, c_long, c_short, c_uint, c_void};

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct __si_common___first___piduid_struct {
        pub si_pid: pid_t,
        pub si_uid: uid_t,
    }
    unsafe impl UserCopyable for __si_common___first___piduid_struct {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct __si_common___first___timer_struct {}
    unsafe impl UserCopyable for __si_common___first___timer_struct {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union __si_common___first_union {
        pub __piduid: __si_common___first___piduid_struct,
        pub __timer: __si_common___first___timer_struct,
    }
    unsafe impl UserCopyable for __si_common___first_union {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct __si_common___second___sigchld_struct {
        pub si_status: c_int,
        pub si_utime: clock_t,
        pub si_stime: clock_t,
    }
    unsafe impl UserCopyable for __si_common___second___sigchld_struct {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union __si_common___second_union {
        pub si_value: sigval,
        pub __sigchld: __si_common___second___sigchld_struct,
    }
    unsafe impl UserCopyable for __si_common___second_union {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct __si_common_struct {
        pub __first: __si_common___first_union,
        pub __second: __si_common___second_union,
    }
    unsafe impl UserCopyable for __si_common_struct {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct __sigfault___first___addr_bnd_struct {
        pub si_lower: *mut c_void,
        pub si_upper: *mut c_void,
    }
    unsafe impl UserCopyable for __sigfault___first___addr_bnd_struct {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union __sigfault___first_union {
        pub __addr_bnd: __sigfault___first___addr_bnd_struct,
        pub si_pkey: c_uint,
    }
    unsafe impl UserCopyable for __sigfault___first_union {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct __sigfault_struct {
        pub si_addr: *mut c_void,
        pub si_addr_lsb: c_short,
        pub __first: __sigfault___first_union,
    }
    unsafe impl UserCopyable for __sigfault_struct {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct __sigpoll_struct {
        pub si_band: c_long,
        pub si_fd: c_long,
    }
    unsafe impl UserCopyable for __sigpoll_struct {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct __sigsys_struct {
        pub si_call_addr: *mut c_void,
        pub si_syscall: c_int,
        pub si_arch: c_uint,
    }
    unsafe impl UserCopyable for __sigsys_struct {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union __si_field_union {
        pub __pad: [c_char; 128 - 2 * size_of::<c_int>() - size_of::<c_long>()],
        pub __si_common: __si_common_struct,
        pub __sigfault: __sigfault_struct,
        pub __sigpoll: __sigpoll_struct,
        pub __sigsys: __sigsys_struct,
    }
    unsafe impl UserCopyable for __si_field_union {}
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct siginfo_t {
    pub si_signo: c_int,
    pub si_errno: c_int,
    pub si_code: c_int,
    pub __si_fields: siginfo::__si_field_union,
}
unsafe impl UserCopyable for siginfo_t {}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum Signal {
    SIGHUP = 1,
    SIGINT = 2,
    SIGQUIT = 3,
    SIGILL = 4,
    SIGTRAP = 5,
    SIGABRT = 6,
    SIGBUS = 7,
    SIGFPE = 8,
    SIGKILL = 9,
    SIGUSR1 = 10,
    SIGSEGV = 11,
    SIGUSR2 = 12,
    SIGPIPE = 13,
    SIGALRM = 14,
    SIGTERM = 15,
    SIGSTKFLT = 16,
    SIGCHLD = 17,
    SIGCONT = 18,
    SIGSTOP = 19,
    SIGTSTP = 20,
    SIGTTIN = 21,
    SIGTTOU = 22,
    SIGURG = 23,
    SIGXCPU = 24,
    SIGXFSZ = 25,
    SIGVTALRM = 26,
    SIGPROF = 27,
    SIGWINCH = 28,
    SIGPOLL = 29,
    SIGPWR = 30,
    SIGSYS = 31,
    SIGCANCEL = 32,
    SIGTIMER = 33,
    SIGRTMIN = 35,
    SIGRTMAX = 64,
}

pub const NSIG: i32 = 65;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct __stack {
    pub ss_sp: *mut c_void,
    pub ss_flags: c_int,
    pub ss_size: usize,
}
unsafe impl UserCopyable for __stack {}
pub type stack_t = __stack;

#[repr(C)]
#[derive(Clone, Copy)]
pub union __sa_handler_union {
    pub sa_handler: *const fn(c_int),
    pub sa_sigaction: *const fn(c_int, *mut siginfo_t, *mut c_void),
}
unsafe impl UserCopyable for __sa_handler_union {}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct sigaction {
    pub __sa_handler: __sa_handler_union,
    pub sa_flags: c_ulong,
    pub sa_restorer: *const fn(),
    pub sa_mask: sigset_t,
}
unsafe impl UserCopyable for sigaction {}
