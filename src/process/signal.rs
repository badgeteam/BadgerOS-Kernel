// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{ffi::c_void, ptr::null};

use crate::{
    bindings::{error::Errno, log::LogLevel},
    cpu::{
        thread::{GpRegfile, SpRegfile},
        usermode::enter_signal,
    },
    kernel::sched::Thread,
    process,
};

use super::uapi::{signal::*, sigset::sigset_t};

/// A process' signal handler table.
#[derive(Clone, Copy)]
pub struct Sigtab {
    pub(super) table: [sigaction; NSIG as usize],
}

impl Default for Sigtab {
    fn default() -> Self {
        Self {
            table: [sigaction {
                __sa_handler: __sa_handler_union {
                    sa_handler: SIG_DFL as *const fn(i32),
                },
                sa_flags: 0,
                sa_restorer: null(),
                sa_mask: sigset_t::default(),
            }; _],
        }
    }
}

/// Run the handler for a segmentation fault.
pub fn run_sigsegv_handler(regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    let v2p = process::current()
        .unwrap()
        .memmap()
        .virt2phys(sregs.is_mem_trap().unwrap_or(0));
    logkf!(
        LogLevel::Debug,
        "SIGSEGV: 0x{:x}, memory is {:x?}",
        sregs.is_mem_trap().unwrap_or(0),
        v2p
    );
    run_handler(
        siginfo_t {
            si_signo: Signal::SIGSEGV as i32,
            si_code: 0,
            si_errno: Errno::EFAULT as i32,
            __si_fields: siginfo::__si_field_union {
                __sigfault: siginfo::__sigfault_struct {
                    si_addr: sregs.is_mem_trap().unwrap_or(0) as *mut c_void,
                    si_addr_lsb: 0,
                    __first: siginfo::__sigfault___first_union { si_pkey: 0 },
                },
            },
        },
        regs,
        sregs,
    );
}

/// Run the handler for an illegal instruction fault.
pub fn run_sigill_handler(regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    logkf!(LogLevel::Debug, "SIGILL: 0x{:x}", sregs.fault_pc());
    run_handler(
        siginfo_t {
            si_signo: Signal::SIGILL as i32,
            si_code: 0,
            si_errno: Errno::EFAULT as i32,
            __si_fields: siginfo::__si_field_union {
                __sigfault: siginfo::__sigfault_struct {
                    si_addr: sregs.fault_pc() as *mut c_void,
                    si_addr_lsb: 0,
                    __first: siginfo::__sigfault___first_union { si_pkey: 0 },
                },
            },
        },
        regs,
        sregs,
    );
}

/// Run the handler for a breakpoint trap.
pub fn run_sigtrap_handler(regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    logkf!(LogLevel::Debug, "SIGTRAP: 0x{:x}", sregs.fault_pc());
    run_handler(
        siginfo_t {
            si_signo: Signal::SIGTRAP as i32,
            si_code: 0,
            si_errno: Errno::EFAULT as i32,
            __si_fields: siginfo::__si_field_union {
                __sigfault: siginfo::__sigfault_struct {
                    si_addr: sregs.fault_pc() as *mut c_void,
                    si_addr_lsb: 0,
                    __first: siginfo::__sigfault___first_union { si_pkey: 0 },
                },
            },
        },
        regs,
        sregs,
    );
}

/// Run the handler for some signal.
pub fn run_handler(siginfo: siginfo_t, regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    logkf!(LogLevel::Debug, "regs:\n{}", regs);
    logkf!(LogLevel::Debug, "sregs:\n{}", sregs);
    if siginfo.si_signo == Signal::SIGKILL as i32 {
        // SIGKILL always kills the process; installing a handler does nothing.
        signal_die(siginfo.si_signo);
        return;
    }

    let proc = super::current().unwrap();
    let guard = proc.sigtab.unintr_lock_shared();
    let action = guard.table[siginfo.si_signo as usize];
    let handler = unsafe { action.__sa_handler.sa_handler as *mut c_void };
    let returner = action.sa_restorer as usize;

    if handler == SIG_DFL {
        use super::uapi::signal::Signal::*;
        match unsafe { core::mem::transmute(siginfo.si_signo) } {
            SIGABRT | SIGBUS | SIGFPE | SIGILL | SIGQUIT | SIGSEGV | SIGSYS | SIGTRAP | SIGXCPU
            | SIGXFSZ => signal_die(siginfo.si_signo), //TODO: With core dump.
            SIGALRM | SIGHUP | SIGINT | SIGPIPE | SIGPROF | SIGPWR | SIGSTKFLT | SIGTERM
            | SIGUSR1 | SIGUSR2 | SIGVTALRM => signal_die(siginfo.si_signo),
            SIGSTOP | SIGTTIN | SIGTTOU => {
                logkf!(LogLevel::Warning, "TODO: Signals stopping the process")
            }
            SIGCONT => logkf!(LogLevel::Warning, "TODO: Signals continuing the process"),
            _ => (), // Other signals ignore by default.
        }
        return;
    } else if handler == SIG_IGN {
        use super::uapi::signal::Signal::*;
        match unsafe { core::mem::transmute(siginfo.si_signo) } {
            SIGSEGV | SIGTRAP | SIGILL | SIGFPE => {
                if unsafe { siginfo.__si_fields.__si_common.__first.__piduid.si_pid } != proc.pid {
                    // Sent by another process, allowed to ignore.
                    return;
                }
                // Can't ignore synchronous traps.
                signal_die(siginfo.si_signo);
            }
            _ => (),
        }
        return;
    }

    let res = unsafe { enter_signal(siginfo, handler as usize, returner, regs, sregs) };
    if res.is_err() {
        signal_die(Signal::SIGSEGV as i32);
        return;
    }

    let runtime = unsafe { (&*Thread::current()).runtime() };
    if (action.sa_flags & SA_NODEFER) == 0 {
        runtime.sigprocmask.set(siginfo.si_code as usize);
    }
    runtime.sigprocmask.add(&action.sa_mask);
}

/// Kill the current process due to a signal.
pub fn signal_die(signal: i32) {
    let proc = super::current().unwrap();
    // W_SIGNALLED
    let status = (signal << 8) | 0x40;
    proc.die(status);
    unsafe { (&*Thread::current()).die() };
}
