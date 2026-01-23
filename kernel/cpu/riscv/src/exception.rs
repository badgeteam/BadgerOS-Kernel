// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{hint::unreachable_unchecked, ptr::null_mut};

use crate::{
    bindings::{device::HasBaseDevice, raw::irqno_t},
    config,
    cpu::{
        self, irq,
        thread::{GpRegfile, SpRegfile},
        usermode::exit_usermode,
    },
    kernel::{
        cpulocal::CpuLocal,
        sched::{Scheduler, Thread},
    },
    mem::vmm::{self},
    misc::panic::unhandled_trap,
    process::{self, syscall},
};

/// An entry in the `.noexc_table`.
#[repr(C)]
#[derive(Clone, Copy)]
struct NoexcEntry {
    start: usize,
    end: usize,
}

unsafe extern "C" {
    static __start_noexc: NoexcEntry;
    static __stop_noexc: NoexcEntry;
}

/// Instruction address misaligned.
pub const CAUSE_IALIGN: isize = 0;
/// Instruction access fault.
pub const CAUSE_IACCESS: isize = 1;
/// Illegal instruction.
pub const CAUSE_IILLEGAL: isize = 2;
/// Breakpoint.
pub const CAUSE_EBREAK: isize = 3;
/// Load address misaligned.
pub const CAUSE_LALIGN: isize = 4;
/// Load access fault.
pub const CAUSE_LACCESS: isize = 5;
/// Store address misaligned.
pub const CAUSE_SALIGN: isize = 6;
/// Store access fault.
pub const CAUSE_SACCESS: isize = 7;
/// E-call from U-mode.
pub const CAUSE_ECALL_U: isize = 8;
/// E-call from S-mode.
pub const CAUSE_ECALL_S: isize = 9;
/// Instruction page fault.
pub const CAUSE_IPAGE: isize = 12;
/// Load page fault.
pub const CAUSE_LPAGE: isize = 13;
/// Store page fault.
pub const CAUSE_SPAGE: isize = 15;
/// Software check.
pub const CAUSE_SWCHK: isize = 18;
/// Hardware error.
pub const CAUSE_HWERR: isize = 19;

/// RISC-V exception handler wrapper.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn riscv_exception_handler(regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    unsafe {
        // Ensure that recursive traps and/or interrupts use the current SP.
        let cpulocal = CpuLocal::get();
        let old_irq_stack = (*cpulocal).arch.irq_stack;
        (*cpulocal).arch.irq_stack = null_mut();
        if let Some(thread) = (*cpulocal).thread.as_deref() {
            thread.runtime().irq_stack = null_mut();
        }
        riscv_exception_handler_impl(regs, sregs);
        if let Some(thread) = (*cpulocal).thread.as_deref() {
            thread.runtime().irq_stack = old_irq_stack;
        }
        (*cpulocal).arch.irq_stack = old_irq_stack;
    }
}

unsafe fn riscv_exception_handler_impl(regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    if sregs.scause < 0 && sregs.scause & 0xff == 5 {
        // Timer interrupt.
        unsafe {
            (*Scheduler::get()).tick_interrupt(!sregs.is_kernel_mode());
        }
        return;
    } else if sregs.scause < 0 {
        // Other interrups.
        unsafe {
            let cpulocal = &mut *CpuLocal::get();
            let handled = cpulocal
                .irqctl
                .as_ref()
                .expect("Missing interrupt controller")
                .interrupt(sregs.scause as irqno_t);
            if !handled {
                unhandled_trap(regs, sregs);
            }
        }
        return;
    }

    let thread = Thread::current();
    if !thread.is_null() && unsafe { &*thread }.is_stopping() && !sregs.is_kernel_mode() {
        // Request to stop thread; exit usermode so the thread can then stop.
        exit_usermode(regs, sregs);
        return;
    }

    if sregs.scause == 8 {
        // ECALL from U-mode.
        let sched = unsafe { &mut *Scheduler::get() };
        sched.account_time(true);
        regs.pc += 4;

        unsafe { irq::enable() };
        syscall::dispatch(
            regs,
            sregs,
            [regs.a0, regs.a1, regs.a2, regs.a3, regs.a4, regs.a5],
            regs.a7,
        );
        unsafe { irq::disable() };

        sched.account_time(false);
        return;
    }

    // Demand paging.
    if let CAUSE_IPAGE | CAUSE_LPAGE | CAUSE_SPAGE = sregs.scause {
        let is_sum = cpu::mmu::check_sum();
        if vmm::page_fault(
            sregs.fault_vaddr() / config::PAGE_SIZE as usize,
            match sregs.scause {
                CAUSE_IPAGE => vmm::flags::X,
                CAUSE_LPAGE => vmm::flags::R,
                CAUSE_SPAGE => vmm::flags::W,
                _ => unsafe { unreachable_unchecked() },
            } + !sregs.is_kernel_mode() as u32 * vmm::flags::U,
            is_sum && sregs.scause != CAUSE_IPAGE,
        ) {
            return;
        }
    }

    // Fallible instructions.
    if sregs.is_kernel_mode() {
        unsafe {
            let mut cur = &raw const __start_noexc;
            while cur != &raw const __stop_noexc {
                if (*cur).start == sregs.sepc {
                    sregs.sepc = (*cur).end;
                    regs.a0 = 1;
                    return;
                }
                cur = cur.wrapping_add(1);
            }
        }
    }

    // Synchronous signals.
    if process::current().is_some() && !sregs.is_kernel_mode() {
        match sregs.scause {
            // Memory access alignment fault.
            CAUSE_IALIGN | CAUSE_LALIGN | CAUSE_SALIGN | CAUSE_IPAGE | CAUSE_LPAGE
            | CAUSE_SPAGE => {
                process::signal::run_sigsegv_handler(regs, sregs);
                return;
            }
            // Illegal instruction fault.
            CAUSE_IILLEGAL => {
                process::signal::run_sigill_handler(regs, sregs);
                return;
            }
            // Breakpoint instruction.
            CAUSE_EBREAK => {
                process::signal::run_sigtrap_handler(regs, sregs);
                return;
            }
            _ => (),
        }
    }

    // If all else fails, kernel panic.
    unhandled_trap(regs, sregs);
}

/// Run an ASM instruction and return true if it causes an exception.
#[macro_export]
macro_rules! noexc_asm {
    (
        $code: literal
        $(, $($params: tt)+)?
    ) => {{
        use core::arch::asm;
        let exc: usize;
        asm!{
            // This will be set to 1 by the exception handler when it detects that the fallible instructions faulted.
            "li a0, 0",
            ".equ __noexc_asm_start, .",
            $code, // Actual instruction to check.
            ".equ __noexc_asm_end, .",
            // This adds it to the table of fallible instructions.
            ".pushsection \".noexc_table\", \"a\", @progbits",
            ".dword __noexc_asm_start",
            ".dword __noexc_asm_end",
            ".popsection"
            // Optional extra in/outs, options, etc.
            $(, $($params)+)?
            // Return value.
            , out("a0") exc
        }
        exc != 0
    }};
}
