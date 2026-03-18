// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#![allow(non_camel_case_types)]

use crate::process::usercopy::UserCopyable;
use core::ffi::{c_int, c_ulong, c_void};

use super::sigset::sigset_t;

#[cfg(target_arch = "riscv64")]
mod uctx_riscv;
#[cfg(target_arch = "riscv64")]
pub use uctx_riscv::*;

pub type __sighandler = *const fn(c_int);

pub const SIG_ERR: *mut c_void = usize::MAX as *mut c_void;
pub const SIG_DFL: *mut c_void = 0 as *mut c_void;
pub const SIG_IGN: *mut c_void = 1 as *mut c_void;

pub const SIG_BLOCK: c_int = 0;
pub const SIG_UNBLOCK: c_int = 1;
pub const SIG_SETMASK: c_int = 2;

pub const SA_NOCLDSTOP: c_ulong = 1;
pub const SA_NOCLDWAIT: c_ulong = 2;
pub const SA_SIGINFO: c_ulong = 4;
pub const SA_ONSTACK: c_ulong = 0x08000000;
pub const SA_RESTART: c_ulong = 0x10000000;
pub const SA_NODEFER: c_ulong = 0x40000000;
pub const SA_RESETHAND: c_ulong = 0x80000000_u32 as _;
pub const SA_RESTORER: c_ulong = 0x04000000;

pub const SI_ASYNCNL: c_int = -60;
pub const SI_TKILL: c_int = -6;
pub const SI_SIGIO: c_int = -5;
pub const SI_ASYNCIO: c_int = -4;
pub const SI_MESGQ: c_int = -3;
pub const SI_TIMER: c_int = -2;
pub const SI_QUEUE: c_int = -1;
pub const SI_USER: c_int = 0;
pub const SI_KERNEL: c_int = 128;

pub mod siginfo {
    use super::super::inttypes::{clock_t, pid_t, uid_t};
    use crate::process::{uapi::sigval::sigval, usercopy::UserCopyable};
    use core::ffi::{c_char, c_int, c_long, c_short, c_uint, c_void};

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct __si_common___first___piduid_struct {
        pub si_pid: pid_t,
        pub si_uid: uid_t,
    }
    unsafe impl UserCopyable for __si_common___first___piduid_struct {}

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct __si_common___first___timer_struct {}
    unsafe impl UserCopyable for __si_common___first___timer_struct {}

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union __si_common___first_union {
        pub __piduid: __si_common___first___piduid_struct,
        pub __timer: __si_common___first___timer_struct,
    }
    impl core::fmt::Debug for __si_common___first_union {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("__si_common___first_union")
                .field("__piduid", unsafe { &self.__piduid })
                .field("__timer", unsafe { &self.__timer })
                .finish()
        }
    }
    unsafe impl UserCopyable for __si_common___first_union {}

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
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
    impl core::fmt::Debug for __si_common___second_union {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("__si_common___second_union")
                .field("si_value", unsafe { &self.si_value })
                .field("__sigchld", unsafe { &self.__sigchld })
                .finish()
        }
    }
    unsafe impl UserCopyable for __si_common___second_union {}

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct __si_common_struct {
        pub __first: __si_common___first_union,
        pub __second: __si_common___second_union,
    }
    unsafe impl UserCopyable for __si_common_struct {}

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
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
    impl core::fmt::Debug for __sigfault___first_union {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("__sigfault___first_union")
                .field("__addr_bnd", unsafe { &self.__addr_bnd })
                .field("si_pkey", unsafe { &self.si_pkey })
                .finish()
        }
    }
    unsafe impl UserCopyable for __sigfault___first_union {}

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct __sigfault_struct {
        pub si_addr: *mut c_void,
        pub si_addr_lsb: c_short,
        pub __first: __sigfault___first_union,
    }
    unsafe impl UserCopyable for __sigfault_struct {}

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct __sigpoll_struct {
        pub si_band: c_long,
        pub si_fd: c_long,
    }
    unsafe impl UserCopyable for __sigpoll_struct {}

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
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
    impl core::fmt::Debug for __si_field_union {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("__si_field_union")
                .field("__si_common", unsafe { &self.__si_common })
                .field("__sigfault", unsafe { &self.__sigfault })
                .field("__sigpoll", unsafe { &self.__sigpoll })
                .field("__sigsys", unsafe { &self.__sigsys })
                .finish()
        }
    }
    impl Default for __si_field_union {
        fn default() -> Self {
            unsafe { core::mem::zeroed() }
        }
    }
    unsafe impl UserCopyable for __si_field_union {}
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct siginfo_t {
    pub si_signo: c_int,
    pub si_errno: c_int,
    pub si_code: c_int,
    pub __si_fields: siginfo::__si_field_union,
}
unsafe impl UserCopyable for siginfo_t {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
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

#[repr(C)]
#[derive(Clone, Copy)]
pub union __sa_handler_union {
    pub sa_handler: *const fn(c_int),
    pub sa_sigaction: *const fn(c_int, *mut siginfo_t, *mut c_void),
}
impl core::fmt::Debug for __sa_handler_union {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("__sa_handler_union")
            .field("sa_handler", unsafe { &self.sa_handler })
            .field("sa_sigaction", unsafe { &self.sa_sigaction })
            .finish()
    }
}
unsafe impl UserCopyable for __sa_handler_union {}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct sigaction {
    pub __sa_handler: __sa_handler_union,
    pub sa_flags: c_ulong,
    pub sa_restorer: *const fn(),
    pub sa_mask: sigset_t,
}
unsafe impl UserCopyable for sigaction {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct stack_t {
    pub ss_sp: *mut c_void,
    pub ss_flags: c_int,
    pub ss_size: usize,
}
unsafe impl UserCopyable for stack_t {}

pub const MINSIGSTKSZ: usize = 2048;
pub const SS_ONSTACK: c_int = 1;
pub const SS_DISABLE: c_int = 2;
