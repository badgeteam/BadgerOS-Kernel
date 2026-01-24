// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    arch::{asm, naked_asm},
    fmt::Display,
    ptr,
    sync::atomic::{Ordering, fence},
};

use alloc::boxed::Box;

use crate::{cpu::irq, kernel::sched::Thread};

/// Special registers state for threads.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SpRegfile {
    pub rip: usize,
    pub cs: usize,
    pub rflags: usize,
    pub rsp: usize,
    pub ss: usize,
    pub err_code: usize,
}

impl SpRegfile {
    pub const fn fault_code(&self) -> isize {
        self.scause
    }

    pub const fn fault_vaddr(&self) -> usize {
        self.stval
    }

    pub const fn fault_pc(&self) -> usize {
        self.sepc
    }

    pub const fn fault_name(&self) -> Option<&'static str> {
        match self.scause {
            0 => Some("Instruction address misaligned"),
            1 => Some("Instruction access fault"),
            2 => Some("Illegal instruction"),
            3 => Some("Breakpoint"),
            4 => Some("Load address misaligned"),
            5 => Some("Load access fault"),
            6 => Some("Store address misaligned"),
            7 => Some("Store access fault"),
            8 => Some("E-call from U-mode"),
            9 => Some("E-call from S-mode"),
            12 => Some("Instruction page fault"),
            13 => Some("Load page fault"),
            15 => Some("Store page fault"),
            18 => Some("Software check"),
            19 => Some("Hardware error"),
            _ => None,
        }
    }

    pub const fn is_mem_trap(&self) -> Option<usize> {
        match self.scause {
            0 | 1 | 4 | 5 | 6 | 7 | 12 | 13 | 15 => Some(self.stval),
            2 | 3 => Some(self.sepc),
            _ => None,
        }
    }

    pub const fn is_kernel_mode(&self) -> bool {
        self.sstatus & 0x100 != 0
    }
}

impl Display for SpRegfile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "  SSTATUS  0x{:x}\n  SCAUSE   0x{:x}\n  STVAL    0x{:x}\n",
            self.sstatus, self.scause, self.stval
        ))
    }
}

/// The general-purpose register, PC, thread pointer and stack.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpRegfile {
    pub rax: usize,
    pub rbx: usize,
    pub rcx: usize,
    pub rdx: usize,
    pub rsi: usize,
    pub rdi: usize,
    pub rsp: usize,
    pub rbp: usize,
    pub r8: usize,
    pub r9: usize,
    pub r10: usize,
    pub r11: usize,
    pub r12: usize,
    pub r13: usize,
    pub r14: usize,
    pub r15: usize,
    pub rip: usize,
}

impl GpRegfile {
    pub fn set_retval(&mut self, val: usize) {
        self.rax = val;
    }

    pub fn set_big_retval(&mut self, val: [usize; 2]) {
        self.rax = val[0];
        self.rdx = val[1];
    }

    pub fn set_pc(&mut self, val: usize) {
        self.rip = val;
    }

    pub fn set_stack(&mut self, val: usize) {
        self.rsp = val;
    }
}

impl Display for GpRegfile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "  RAX 0x{:016x}  RBX 0x{:016x}  RCX 0x{:016x}  RDX 0x{:016x}\n  RSI 0x{:016x}  RDI 0x{:016x}  RSP 0x{:016x}  RBP 0x{:016x}\n  R8  0x{:016x}  R9  0x{:016x}  R10 0x{:016x}  R11 0x{:016x}\n  R12 0x{:016x}  R13 0x{:016x}  R14 0x{:016x}  R15 0x{:016x}\n  RIP 0x{:016x}",
            self.rax,
            self.rbx,
            self.rcx,
            self.rdx,
            self.rsi,
            self.rdi,
            self.rsp,
            self.rbp,
            self.r8,
            self.r9,
            self.r10,
            self.r11,
            self.r12,
            self.r13,
            self.r14,
            self.r15,
            self.rip,
        ))?;

        Ok(())
    }
}

/// Set up the entrypoint for a thread given its kernel stack.
/// Returns how many words of stack were used.
pub fn prepare_entry(stack: &mut [usize], code: Box<dyn FnOnce() + Send + 'static>) -> usize {
    const WORDS: usize = 16;
    let len = stack.len();
    let stack = &mut stack[len - WORDS..];
    stack.fill(0);

    let code = Box::into_raw(code);
    let data = code as *mut ();
    let meta: *const () = unsafe { core::mem::transmute(ptr::metadata(code)) };

    // Entrypoint for trampoline.
    stack[15] = meta as usize;
    stack[14] = data as usize;
    // Return address for `context_switch`.
    stack[12] = thread_trampoline_1 as *const fn() as usize;

    WORDS
}

/// Part 1: Load the raw parts of the `Box<dyn FnOnce()>`.
#[unsafe(naked)]
unsafe extern "C" fn thread_trampoline_1() {
    naked_asm!(
        "ld   a0, 0(sp)",
        "ld   a1, 8(sp)",
        "j    {}",
        sym thread_trampoline_2
    );
}

/// Part 2: Reconstruct and call the `Box<dyn FnOnce()>`.
unsafe extern "C" fn thread_trampoline_2(ptr: *mut (), meta: *mut ()) {
    unsafe {
        let code: *mut dyn FnOnce() = ptr::from_raw_parts_mut(ptr, core::mem::transmute(meta));
        fence(Ordering::Acquire);
        irq::enable();
        Box::from_raw(code)();
        (*Thread::current()).die();
    }
}

/// Switch to another thread context.
#[unsafe(naked)]
pub unsafe extern "C" fn context_switch(new_stack: *const *mut (), old_stack_out: *mut *mut ()) {
    naked_asm!(
        // Save old context to stack.
        "addi sp, sp, -14*8",
        "sd   s0, 8*0(sp)",
        "sd   s1, 8*1(sp)",
        "sd   s2, 8*2(sp)",
        "sd   s3, 8*3(sp)",
        "sd   s4, 8*4(sp)",
        "sd   s5, 8*5(sp)",
        "sd   s6, 8*6(sp)",
        "sd   s7, 8*7(sp)",
        "sd   s8, 8*8(sp)",
        "sd   s9, 8*9(sp)",
        "sd   s10, 8*10(sp)",
        "sd   s11, 8*11(sp)",
        "sd   ra, 8*12(sp)",
        // Swap out stack pointers.
        "sd   sp, 0(a1)",
        "ld   sp, 0(a0)",
        // Restore new context from stack.
        "ld   s0, 8*0(sp)",
        "ld   s1, 8*1(sp)",
        "ld   s2, 8*2(sp)",
        "ld   s3, 8*3(sp)",
        "ld   s4, 8*4(sp)",
        "ld   s5, 8*5(sp)",
        "ld   s6, 8*6(sp)",
        "ld   s7, 8*7(sp)",
        "ld   s8, 8*8(sp)",
        "ld   s9, 8*9(sp)",
        "ld   s10, 8*10(sp)",
        "ld   s11, 8*11(sp)",
        "ld   ra, 8*12(sp)",
        "addi sp, sp, 14*8",
        // Return to the new thread context.
        "ret"
    );
}

/// Run a CPU pause hint instruction.
#[inline(always)]
pub fn pause_hint() {
    // RISC-V Zihintpause instruction.
    // This is a fence with PRED=W and SUCC=none.
    unsafe { asm!(".word 0x0100000f") };
}
