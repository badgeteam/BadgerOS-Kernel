// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::{asm, naked_asm};

use crate::{
    bindings::raw::siginfo_t,
    cpu::{
        irq,
        thread::{GpRegfile, SpRegfile},
    },
    kernel::{cpulocal::CpuLocal, sched::Thread},
};

/// Thread's userspace context.
/// Tells [`exit_usermode`] how to return to the kernel from some exception handler.
#[derive(Default)]
#[repr(C)]
pub struct ThreadUContext {
    pc: usize,
    sp: usize,
    s0: usize,
    s1: usize,
    s2: usize,
    s3: usize,
    s4: usize,
    s5: usize,
    s6: usize,
    s7: usize,
    s8: usize,
    s9: usize,
    s10: usize,
    s11: usize,
}

/// Enter a userspace signal handler.
pub unsafe fn enter_signal(
    info: siginfo_t,
    handler: usize,
    returner: usize,
    regs: &mut GpRegfile,
    sregs: &mut SpRegfile,
) -> bool {
    false
}

/// Exit a userspace signal handler.
pub unsafe fn exit_signal(regs: &mut GpRegfile, sregs: &mut SpRegfile) -> bool {
    false
}

/// Call into userspace from this thread.
pub fn call_usermode(entry: *const (), stack: *mut ()) {
    unsafe {
        let uctx = &mut (&mut *Thread::current()).runtime().uctx;
        debug_assert!(uctx.pc == 0, "Cannot recursively call into usermode");
        irq::disable();

        let cpulocal = &mut *CpuLocal::get();
        let runtime = (&*Thread::current()).runtime();
        enter_usermode_asm(
            entry,
            stack,
            uctx,
            &mut cpulocal.arch.irq_stack,
            &mut runtime.irq_stack,
        );
        // Interrupts re-enabled by `exit_usermode`.
    }
}

#[unsafe(naked)]
unsafe extern "C" fn enter_usermode_asm(
    entry: *const (),
    stack: *mut (),
    savestate: &mut ThreadUContext,
    irq_stack: &mut *mut (),
    irq_stack_2: &mut *mut (),
) {
    naked_asm!(
        // Saving callee-saved regs to uctx.
        "sd ra, 0*8(a2)",
        "sd sp, 1*8(a2)",
        "sd s0, 2*8(a2)",
        "sd s1, 3*8(a2)",
        "sd s2, 4*8(a2)",
        "sd s3, 5*8(a2)",
        "sd s4, 6*8(a2)",
        "sd s5, 7*8(a2)",
        "sd s6, 8*8(a2)",
        "sd s7, 9*8(a2)",
        "sd s8, 10*8(a2)",
        "sd s9, 11*8(a2)",
        "sd s10, 12*8(a2)",
        "sd s11, 13*8(a2)",
        // The old sp is also the new interrupt stack.
        "sd sp, 0(a3)",
        "sd sp, 0(a4)",
        // Setting up for sret into U-mode.
        "li t0, (1<<8)",
        "csrc sstatus, t0", // -SPP
        "li t0, (1<<5)",
        "csrs sstatus, t0", // +SPIE
        "csrw sepc, a0",
        "mv sp, a1",
        "sret"
    );
}

/// Return from userspace in this thread.
/// Overwrites `regs` and `sregs` with the values needed to continue into the kernel.
pub fn exit_usermode(regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    let uctx = unsafe { &mut (&mut *Thread::current()).runtime().uctx };
    sregs.sepc = uctx.pc;
    sregs.sstatus |= (1 << 8) | (1 << 5); // +SPP +SPIE
    *regs = GpRegfile::default();
    regs.sp = uctx.sp;
    regs.s0 = uctx.s0;
    regs.s1 = uctx.s1;
    regs.s2 = uctx.s2;
    regs.s3 = uctx.s3;
    regs.s4 = uctx.s4;
    regs.s5 = uctx.s5;
    regs.s6 = uctx.s6;
    regs.s7 = uctx.s7;
    regs.s8 = uctx.s8;
    regs.s9 = uctx.s9;
    regs.s10 = uctx.s10;
    regs.s11 = uctx.s11;
    unsafe { asm!("mv {}, gp", out(reg)regs.gp) };
}
