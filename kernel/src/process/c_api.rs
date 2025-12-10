// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ffi::{c_int, c_void},
    sync::atomic::AtomicU32,
};

use crate::{
    bindings::{
        self,
        raw::{
            SIGILL, SIGSEGV, SIGSYS, SIGTRAP, isr_context_switch, sched_lower_from_isr, siginfo_t,
        },
    },
    cpu::irq,
    mem::vmm::Memmap,
    process::current,
};

use super::{Process, signal};

/// Needed by C because the process struct is not representable in C.
#[unsafe(no_mangle)]
unsafe extern "C" fn proc_memmap(proc: &Process) -> &Memmap {
    &proc.memmap
}

/// Needed by C because the process struct is not representable in C.
#[unsafe(no_mangle)]
unsafe extern "C" fn proc_flags(proc: &Process) -> &AtomicU32 {
    &proc.flags
}

/// Start the init process.
#[unsafe(no_mangle)]
unsafe extern "C" fn proc_start_init() {
    super::Process::new_init().expect("Failed to start init process");
}

/// Helper that gets the program counter.
pub fn get_user_pc() -> usize {
    unsafe {
        let thread = bindings::raw::sched_current_thread();
        #[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
        {
            (*thread).user_isr_ctx.regs.pc
        }
        #[cfg(target_arch = "x86_64")]
        {
            (*thread).user_isr_ctx.regs.rip
        }
    }
}

/// Called when SIGSEGV is raised by a trap.
#[unsafe(no_mangle)]
unsafe extern "C" fn proc_sigsegv_handler() {
    signal::run_handler(siginfo_t {
        si_signo: SIGSEGV as c_int,
        si_code: 0,
        si_pid: current().unwrap().pid(),
        si_uid: 0,
        si_addr: get_user_pc() as *mut c_void, // TODO: Context switching refactor needed to fix this.
        si_status: 0,
    });
    unsafe {
        irq::disable();
        sched_lower_from_isr();
        isr_context_switch();
    };
}

/// Called when SIGILL is raised by a trap.
#[unsafe(no_mangle)]
unsafe extern "C" fn proc_sigill_handler() {
    signal::run_handler(siginfo_t {
        si_signo: SIGILL as c_int,
        si_code: 0,
        si_pid: current().unwrap().pid(),
        si_uid: 0,
        si_addr: get_user_pc() as *mut c_void,
        si_status: 0,
    });
    unsafe {
        irq::disable();
        sched_lower_from_isr();
        isr_context_switch();
    };
}

/// Called when SIGTRAP is raised by a trap.
#[unsafe(no_mangle)]
unsafe extern "C" fn proc_sigtrap_handler() {
    signal::run_handler(siginfo_t {
        si_signo: SIGTRAP as c_int,
        si_code: 0,
        si_pid: current().unwrap().pid(),
        si_uid: 0,
        si_addr: get_user_pc() as *mut c_void,
        si_status: 0,
    });
    unsafe {
        irq::disable();
        sched_lower_from_isr();
        isr_context_switch();
    };
}

#[unsafe(no_mangle)]
unsafe extern "C" fn proc_sigsys_handler() {
    signal::run_handler(siginfo_t {
        si_signo: SIGSYS as c_int,
        si_code: 0,
        si_pid: current().unwrap().pid(),
        si_uid: 0,
        si_addr: get_user_pc() as *mut c_void,
        si_status: 0,
    });
    unsafe {
        irq::disable();
        sched_lower_from_isr();
        isr_context_switch();
    };
}
