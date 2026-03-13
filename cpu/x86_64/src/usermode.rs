// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::asm;

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
    rip: usize,
    rflags: usize,
    rsp: usize,
    rbp: usize,
    rbx: usize,
    r12: usize,
    r13: usize,
    r14: usize,
    r15: usize,
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
        debug_assert!(uctx.rip == 0, "Cannot recursively call into usermode");
        irq::disable();

        let cpulocal = &mut *CpuLocal::get();
        let runtime = (&*Thread::current()).runtime();
        todo!()
        // Interrupts re-enabled by `exit_usermode`.
    }
}

unsafe extern "C" {
    fn enter_usermode_asm();
}

/// Return from userspace in this thread.
/// Overwrites `regs` and `sregs` with the values needed to continue into the kernel.
pub fn exit_usermode(regs: &mut GpRegfile, sregs: &mut SpRegfile) {
    let uctx = unsafe { &mut (&mut *Thread::current()).runtime().uctx };
    sregs.rip = uctx.rip;
    sregs.rflags = uctx.rflags;
    *regs = GpRegfile::default();
    regs.rip = uctx.rip;
    regs.rsp = uctx.rsp;
    regs.rbp = uctx.rbp;
    regs.rbx = uctx.rbx;
    regs.r12 = uctx.r12;
    regs.r13 = uctx.r13;
    regs.r14 = uctx.r14;
    regs.r15 = uctx.r15;
}
