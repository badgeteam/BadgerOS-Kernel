// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    arch::{asm, naked_asm},
    fmt::Display,
    ptr,
};

use alloc::boxed::Box;
use bytemuck_derive::{AnyBitPattern, NoUninit};

use crate::kernel::sched::Thread;

/// Special registers state for threads.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SpRegfile {
    pub sstatus: usize,
    pub scause: isize,
    pub stval: usize,
    pub sepc: usize,
    pub fake_fp: *const (),
}

impl SpRegfile {
    pub const fn fault_code(&self) -> isize {
        self.scause
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

    pub(crate) fn fault_fake_frame_ptr(&self) -> *const () {
        self.fake_fp
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
#[derive(Clone, Copy, Default, NoUninit, AnyBitPattern)]
pub struct GpRegfile {
    pub pc: usize,
    pub ra: usize,
    pub sp: usize,
    pub gp: usize,
    pub tp: usize,
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,
    pub s0: usize,
    pub s1: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize,
}

impl GpRegfile {
    pub fn set_retval(&mut self, val: usize) {
        self.a0 = val;
    }

    pub fn set_big_retval(&mut self, val: [usize; 2]) {
        self.a0 = val[0];
        self.a1 = val[1];
    }

    pub fn set_pc(&mut self, val: usize) {
        self.pc = val;
    }

    pub fn set_stack(&mut self, val: usize) {
        self.sp = val;
    }

    pub fn get_pc(&self) -> usize {
        self.pc
    }

    pub fn get_stack(&self) -> usize {
        self.sp
    }
}

impl Display for GpRegfile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[cfg(target_arch = "riscv32")]
        f.write_fmt(format_args!(
            "  PC  0x{:08x}  RA  0x{:08x}  SP  0x{:08x}  GP  0x{:08x}\n  TP  0x{:08x}  T0  0x{:08x}  T1  0x{:08x}  T2  0x{:08x}\n  S0  0x{:08x}  S1  0x{:08x}  A0  0x{:08x}  A1  0x{:08x}\n  A2  0x{:08x}  A3  0x{:08x}  A4  0x{:08x}  A5  0x{:08x}\n  A6  0x{:08x}  A7  0x{:08x}  S2  0x{:08x}  S3  0x{:08x}\n  S4  0x{:08x}  S5  0x{:08x}  S6  0x{:08x}  S7  0x{:08x}\n  S8  0x{:08x}  S9  0x{:08x}  S10 0x{:08x}  S11 0x{:08x}\n  T3  0x{:08x}  T4  0x{:08x}  T5  0x{:08x}  T6  0x{:08x}\n",
            self.pc,
            self.ra,
            self.sp,
            self.gp,
            self.tp,
            self.t0,
            self.t1,
            self.t2,
            self.s0,
            self.s1,
            self.a0,
            self.a1,
            self.a2,
            self.a3,
            self.a4,
            self.a5,
            self.a6,
            self.a7,
            self.s2,
            self.s3,
            self.s4,
            self.s5,
            self.s6,
            self.s7,
            self.s8,
            self.s9,
            self.s10,
            self.s11,
            self.t3,
            self.t4,
            self.t5,
            self.t6,
        ))?;

        #[cfg(target_arch = "riscv64")]
        f.write_fmt(format_args!(
            "  PC  0x{:016x}  RA  0x{:016x}  SP  0x{:016x}  GP  0x{:016x}\n  TP  0x{:016x}  T0  0x{:016x}  T1  0x{:016x}  T2  0x{:016x}\n  S0  0x{:016x}  S1  0x{:016x}  A0  0x{:016x}  A1  0x{:016x}\n  A2  0x{:016x}  A3  0x{:016x}  A4  0x{:016x}  A5  0x{:016x}\n  A6  0x{:016x}  A7  0x{:016x}  S2  0x{:016x}  S3  0x{:016x}\n  S4  0x{:016x}  S5  0x{:016x}  S6  0x{:016x}  S7  0x{:016x}\n  S8  0x{:016x}  S9  0x{:016x}  S10 0x{:016x}  S11 0x{:016x}\n  T3  0x{:016x}  T4  0x{:016x}  T5  0x{:016x}  T6  0x{:016x}\n",
            self.pc,
            self.ra,
            self.sp,
            self.gp,
            self.tp,
            self.t0,
            self.t1,
            self.t2,
            self.s0,
            self.s1,
            self.a0,
            self.a1,
            self.a2,
            self.a3,
            self.a4,
            self.a5,
            self.a6,
            self.a7,
            self.s2,
            self.s3,
            self.s4,
            self.s5,
            self.s6,
            self.s7,
            self.s8,
            self.s9,
            self.s10,
            self.s11,
            self.t3,
            self.t4,
            self.t5,
            self.t6,
        ))?;

        Ok(())
    }
}

pub mod xs {
    pub const OFF: usize = 0;
    pub const INIT: usize = 1;
    pub const CLEAN: usize = 2;
    pub const DIRTY: usize = 3;
    pub const MASK: usize = 3;
}

pub const SSTATUS_XS_BIT: u32 = 15;
pub const SSTATUS_FS_BIT: u32 = 13;
pub const SSTATUS_VS_BIT: u32 = 9;

/// The floating-point register state.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FloatState {
    pub(super) fregs: [u64; 32],
    // Note: `fflags` and `frm` are part of `fcsr`.
    pub(super) fcsr: usize,
    /// Whether this runtime's floating-point state is currently enabled.
    pub(super) float_enable: bool,
}

impl FloatState {
    pub fn new() -> Self {
        Self {
            fregs: [0u64; 32],
            fcsr: 0,
            float_enable: false,
        }
    }

    /// Load the state of the floating-point register file.
    pub unsafe fn load_state(&self, sregs: &mut SpRegfile) {
        if !self.float_enable {
            return;
        }
        unsafe {
            asm!("csrs sstatus, {}", in(reg) xs::DIRTY << SSTATUS_FS_BIT);
            asm!(
                ".option push",
                ".option arch, +d",
                ".rept 32",
                "fld f\\+, \\+*8({})",
                ".endr",
                ".option pop",
                in(reg) self
            );
            asm!("csrc sstatus, {}", in(reg) xs::DIRTY << SSTATUS_FS_BIT);
            sregs.sstatus &= !(xs::MASK << SSTATUS_FS_BIT);
            sregs.sstatus |= xs::CLEAN << SSTATUS_FS_BIT;
        }
    }

    /// Save the state of the floating-point register file.
    pub unsafe fn save_state(&mut self, sregs: &SpRegfile) {
        if !self.float_enable {
            return;
        }
        match (sregs.sstatus >> SSTATUS_FS_BIT) & 3 {
            xs::INIT | xs::CLEAN => return,
            _ => (),
        }
        unsafe {
            asm!("csrs sstatus, {}", in(reg) xs::DIRTY << SSTATUS_FS_BIT);
            asm!(
                ".option push",
                ".option arch, +d",
                "csrr {}, fcsr",
                ".rept 32",
                "fsd f\\+, \\+*8({})",
                ".endr",
                ".option pop",
                in(reg) self,
                out(reg) self.fcsr
            );
            asm!("csrc sstatus, {}", in(reg) xs::DIRTY << SSTATUS_FS_BIT);
        }
    }

    /// Initially enable floating-point state.
    pub(super) unsafe fn enable(&mut self, sregs: &mut SpRegfile) {
        debug_assert!(!self.float_enable);
        sregs.sstatus |= xs::INIT << SSTATUS_FS_BIT;
        unsafe {
            asm!("csrs sstatus, {}", in(reg) xs::DIRTY << SSTATUS_FS_BIT);
            asm!(
                ".option push",
                ".option arch, +d",
                ".rept 32",
                "fmv.d.x f\\+, x0",
                ".endr",
                "csrw fcsr, {}",
                ".option pop",
                in(reg) self.fcsr
            );
            asm!("csrc sstatus, {}", in(reg) xs::DIRTY << SSTATUS_FS_BIT);
        }
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
    stack[12] = Thread::thread_trampoline_1 as *const fn() as usize;

    WORDS
}

impl Thread {
    /// Part 1: Load the raw parts of the `Box<dyn FnOnce()>`.
    #[unsafe(naked)]
    #[cfg(target_arch = "riscv64")]
    pub unsafe extern "C" fn thread_trampoline_1() {
        naked_asm!(
            "ld   a1, 0(sp)",
            "ld   a2, 8(sp)",
            "j    {}",
            sym Thread::thread_trampoline_2
        );
    }
}

/// Switch to another thread context.
#[unsafe(naked)]
#[cfg(target_arch = "riscv64")]
pub unsafe extern "C" fn context_switch<T>(
    passthru: *const T,
    new_stack: *const *mut (),
    old_stack_out: *mut *mut (),
) -> *const T {
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
        "sd   sp, 0(a2)",
        "ld   sp, 0(a1)",
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
