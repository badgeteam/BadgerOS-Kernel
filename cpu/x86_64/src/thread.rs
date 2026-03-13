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
    pub irq: isize,
    pub cr2: usize,
}

impl SpRegfile {
    pub const fn fault_code(&self) -> isize {
        self.irq
    }

    pub const fn fault_pc(&self) -> usize {
        self.rip
    }

    pub const fn fault_name(&self) -> Option<&'static str> {
        match self.irq {
            0 => Some("Division by zero"),
            1 => Some("Debug exception"),
            2 => Some("Non-maskable interrupt"),
            3 => Some("Breakpoint"),
            4 => Some("Integer overflow"),
            5 => Some("Bound range exceeded"),
            6 => Some("Illegal instruction"),
            8 => Some("Double fault"),
            10 => Some("Invalid TSS"),
            11 => Some("Segment not present"),
            12 => Some("Stack-segment fault"),
            13 => Some("General protection fault"),
            14 => Some("Page fault"),
            17 => Some("Alignment check"),
            18 => Some("Machine check"),
            19 => Some("SIMD floating-point exception"),
            20 => Some("Virtualization exception"),
            21 => Some("Control protection exception"),
            _ => None,
        }
    }

    pub const fn is_mem_trap(&self) -> Option<usize> {
        match self.irq {
            14 => Some(self.cr2), // Page fault
            _ => None,
        }
    }

    pub const fn is_kernel_mode(&self) -> bool {
        self.cs & 3 == 0
    }
}

impl Display for SpRegfile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "  RIP    0x{:016x}\n  CS     0x{:04x}\n  RFLAGS 0x{:016x}\n  RSP    0x{:016x}\n  SS     0x{:04x}\n  ERR#   0x{:016x}\n  IRQ#   0x{:02x}\n  CR2    0x{:016x}",
            self.rip,
            self.cs,
            self.rflags,
            self.rsp,
            self.ss,
            self.err_code,
            self.irq,
            self.cr2,
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
    pub fn get_pc(&self) -> usize {
        self.rip
    }

    pub fn get_stack(&self) -> usize {
        self.rsp
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
    const WORDS: usize = 10;
    let len = stack.len();
    let stack = &mut stack[len - WORDS..];
    stack.fill(0);

    let code = Box::into_raw(code);
    let data = code as *mut ();
    let meta: *const () = unsafe { core::mem::transmute(ptr::metadata(code)) };

    // Entrypoint for trampoline.
    stack[8] = meta as usize;
    stack[7] = data as usize;
    // Return address for `context_switch`.
    stack[6] = thread_trampoline_1 as *const fn() as usize;

    WORDS
}

/// Part 1: Load the raw parts of the `Box<dyn FnOnce()>`.
#[unsafe(naked)]
unsafe extern "C" fn thread_trampoline_1() {
    naked_asm!(
        "mov rdi, [rsp+0]",
        "mov rsi, [rsp+8]",
        "jmp {}",
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
        "push rbp",
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push rbx",
        // Swap out stack pointers.
        "mov [rsi], rsp",
        "mov rsp, [rdi]",
        // Restore new context from stack.
        "pop rbx",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",
        "pop rbp",
        // Return to the new thread context.
        "ret"
    );
}

/// Run a CPU pause hint instruction.
#[inline(always)]
pub fn pause_hint() {
    unsafe { asm!("pause", options(nomem, preserves_flags)) };
}
