// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ffi::c_void;

use crate::{
    bindings::{
        log::LogLevel,
        raw::{sigaction, sigaction__bindgen_ty_1, siginfo_t},
    },
    cpu::{
        thread::{GpRegfile, SpRegfile},
        usermode::enter_signal,
    },
    kernel::sched::Thread,
    process::current,
};

/// Signal numbers recognised by BadgerOS.
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
    SIGIO = 29,
    SIGPWR = 30,
    SIGSYS = 31,
}
pub const SIG_IGN: usize = 0;
pub const SIG_DFL: usize = usize::MAX;
pub const SIG_COUNT: i32 = 32;

/// A process' signal handler table.
#[derive(Clone, Copy)]
pub struct Sigtab {
    pub(super) table: [sigaction; SIG_COUNT as usize],
}

impl Default for Sigtab {
    fn default() -> Self {
        Self {
            table: [sigaction {
                __bindgen_anon_1: sigaction__bindgen_ty_1 {
                    sa_handler_ptr: SIG_DFL as *mut c_void,
                },
                sa_mask: 0,
                sa_flags: 0,
                sa_return_trampoline: 0 as *mut c_void,
            }; _],
        }
    }
}

/// Run the handler for a segmentation fault.
pub fn run_sigsegv_handler(regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    run_handler(
        siginfo_t {
            si_signo: Signal::SIGSEGV as i32,
            si_code: 0,
            si_pid: current().unwrap().pid,
            si_uid: 0,
            si_addr: sregs.is_mem_trap().unwrap_or(0) as *mut c_void,
            si_status: 0,
        },
        regs,
        sregs,
    );
}

/// Run the handler for an illegal instruction fault.
pub fn run_sigill_handler(regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    run_handler(
        siginfo_t {
            si_signo: Signal::SIGILL as i32,
            si_code: 0,
            si_pid: current().unwrap().pid,
            si_uid: 0,
            si_addr: sregs.fault_pc() as *mut c_void,
            si_status: 0,
        },
        regs,
        sregs,
    );
}

/// Run the handler for a breakpoint trap.
pub fn run_sigtrap_handler(regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    run_handler(
        siginfo_t {
            si_signo: Signal::SIGTRAP as i32,
            si_code: 0,
            si_pid: current().unwrap().pid,
            si_uid: 0,
            si_addr: sregs.fault_pc() as *mut c_void,
            si_status: 0,
        },
        regs,
        sregs,
    );
}

/// Run the handler for some signal.
pub fn run_handler(siginfo: siginfo_t, regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    logkf!(
        LogLevel::Debug,
        "Running signal handler for {:#?}",
        &siginfo
    );
    if siginfo.si_signo == Signal::SIGKILL as i32 {
        // SIGKILL always kills the process; installing a handler does nothing.
        signal_die(siginfo.si_signo);
        return;
    }

    let proc = super::current().unwrap();
    let guard = proc.sigtab.unintr_lock_shared();
    let action = guard.table[siginfo.si_signo as usize];
    let handler = unsafe { action.__bindgen_anon_1.sa_handler_ptr } as usize;
    let returner = action.sa_return_trampoline as usize;

    if handler == SIG_DFL {
        use Signal::*;
        match unsafe { core::mem::transmute(siginfo.si_signo) } {
            SIGABRT | SIGBUS | SIGFPE | SIGILL | SIGQUIT | SIGSEGV | SIGSYS | SIGTRAP | SIGXCPU
            | SIGXFSZ => signal_die(siginfo.si_signo), //TODO: With core dump.
            SIGALRM | SIGHUP | SIGINT | SIGIO | SIGPIPE | SIGPROF | SIGPWR | SIGSTKFLT
            | SIGTERM | SIGUSR1 | SIGUSR2 | SIGVTALRM => signal_die(siginfo.si_signo),
            SIGSTOP | SIGTTIN | SIGTTOU => {
                logkf!(LogLevel::Warning, "TODO: Signals stopping the process")
            }
            SIGCONT => logkf!(LogLevel::Warning, "TODO: Signals continuing the process"),
            _ => (), // Other signals ignore by default.
        }
        return;
    } else if handler == SIG_IGN {
        if siginfo.si_pid != proc.pid {
            // Sent by another process, allowed to ignore.
            return;
        }
        match unsafe { core::mem::transmute(siginfo.si_signo) } {
            Signal::SIGSEGV | Signal::SIGTRAP | Signal::SIGILL | Signal::SIGFPE => {
                // Can't ignore synchronous traps.
                signal_die(siginfo.si_signo);
            }
            _ => (),
        }
        return;
    }

    // TODO: Get rid of sched_signal_enter?
    let success = unsafe { enter_signal(siginfo, handler, returner, regs, sregs) };
    if !success {
        signal_die(Signal::SIGSEGV as i32);
    }
}

/// Kill the current process due to a signal.
pub fn signal_die(signal: i32) {
    let proc = super::current().unwrap();
    // W_SIGNALLED
    let status = (signal << 8) | 0x40;
    proc.kill(status);
    unsafe { (&*Thread::current()).die() };
}
