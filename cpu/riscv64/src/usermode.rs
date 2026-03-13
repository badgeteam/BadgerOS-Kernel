// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::asm;

use crate::{
    cpu::{
        irq,
        thread::{GpRegfile, SpRegfile},
    },
    kernel::{cpulocal::CpuLocal, sched::Thread},
    process::uapi::signal::siginfo_t,
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
pub fn call_usermode(regs: &GpRegfile) {
    unsafe {
        let uctx = &mut (&mut *Thread::current()).runtime().uctx;
        debug_assert!(uctx.pc == 0, "Cannot recursively call into usermode");
        irq::disable();

        let cpulocal = &mut *CpuLocal::get();
        let runtime = (&*Thread::current()).runtime();
        enter_usermode_asm(
            regs,
            uctx,
            &mut cpulocal.arch.irq_stack,
            &mut runtime.irq_stack,
        );
        // Interrupts re-enabled by `exit_usermode`.
    }
}

unsafe extern "C" {
    fn enter_usermode_asm(
        regs: &GpRegfile,
        savestate: &mut ThreadUContext,
        irq_stack: &mut *mut (),
        irq_stack_2: &mut *mut (),
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
