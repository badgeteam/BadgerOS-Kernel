use core::{
    arch::{asm, naked_asm},
    mem::offset_of,
};

use crate::{
    arch::{kcore::cpulocal::ArchCpuLocal, riscv64::csr, usermode::*},
    bindings::error::Errno,
    kcore::sched::Thread,
    process::{
        uapi::signal::siginfo_t,
        usercopy::{AccessFault, AccessResult},
    },
};

use super::{Riscv, RiscvRegfile, RiscvSavedRegs, except::RiscvExceptFrame};

/// Run an ASM instruction and return true if it causes an exception.
macro_rules! noexc_asm {
    (
        $code: literal
        $(, $($params: tt)+)?
    ) => {{
        let exc: usize;
        core::arch::asm!{
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

#[unsafe(naked)]
unsafe extern "C" fn enter_usermode_impl(
    thread_irq_stack_out: &mut *mut (),
    cpulocal_irq_stack_out: &mut *mut (),
    save: &mut RiscvSavedRegs,
    load: &RiscvRegfile,
) {
    naked_asm!(
        // Save kernel regs.
        "sd ra, {save_pc}(a2)
        sd sp, 0(a0)
        sd sp, 0(a1)
        sd sp, {save_sp}(a2)
        sd s0, {save_s0}(a2)
        sd s1, {save_s1}(a2)
        sd s2, {save_s2}(a2)
        sd s3, {save_s3}(a2)
        sd s4, {save_s4}(a2)
        sd s5, {save_s5}(a2)
        sd s6, {save_s6}(a2)
        sd s7, {save_s7}(a2)
        sd s8, {save_s8}(a2)
        sd s9, {save_s9}(a2)
        sd s10, {save_s10}(a2)
        sd s11, {save_s11}(a2)",
        // Load user regs.
        "ld t0, {load_pc}(a3)
        csrw sepc, t0
        ld ra, {load_ra}(a3)
        ld sp, {load_sp}(a3)
        ld gp, {load_gp}(a3)
        ld tp, {load_tp}(a3)
        ld t0, {load_t0}(a3)
        ld t1, {load_t1}(a3)
        ld t2, {load_t2}(a3)
        ld s0, {load_s0}(a3)
        ld s1, {load_s1}(a3)
        ld a0, {load_a0}(a3)
        ld a1, {load_a1}(a3)
        ld a2, {load_a2}(a3)
        ld a4, {load_a4}(a3)
        ld a5, {load_a5}(a3)
        ld a6, {load_a6}(a3)
        ld a7, {load_a7}(a3)
        ld s2, {load_s2}(a3)
        ld s3, {load_s3}(a3)
        ld s4, {load_s4}(a3)
        ld s5, {load_s5}(a3)
        ld s6, {load_s6}(a3)
        ld s7, {load_s7}(a3)
        ld s8, {load_s8}(a3)
        ld s9, {load_s9}(a3)
        ld s10, {load_s10}(a3)
        ld s11, {load_s11}(a3)
        ld t3, {load_t3}(a3)
        ld t4, {load_t4}(a3)
        ld t5, {load_t5}(a3)
        ld t6, {load_t6}(a3)
        ld a3, {load_a3}(a3)",
        // Enter U-mode.
        // We will return to the caller whenever `exit_usermode` gets called by the interrupt handler.
        "sret",

        save_pc = const offset_of!(RiscvSavedRegs, pc),
        save_sp = const offset_of!(RiscvSavedRegs, sp),
        save_s0 = const offset_of!(RiscvSavedRegs, s0),
        save_s1 = const offset_of!(RiscvSavedRegs, s1),
        save_s2 = const offset_of!(RiscvSavedRegs, s2),
        save_s3 = const offset_of!(RiscvSavedRegs, s3),
        save_s4 = const offset_of!(RiscvSavedRegs, s4),
        save_s5 = const offset_of!(RiscvSavedRegs, s5),
        save_s6 = const offset_of!(RiscvSavedRegs, s6),
        save_s7 = const offset_of!(RiscvSavedRegs, s7),
        save_s8 = const offset_of!(RiscvSavedRegs, s8),
        save_s9 = const offset_of!(RiscvSavedRegs, s9),
        save_s10 = const offset_of!(RiscvSavedRegs, s10),
        save_s11 = const offset_of!(RiscvSavedRegs, s11),

        load_pc = const offset_of!(RiscvRegfile, pc),
        load_ra = const offset_of!(RiscvRegfile, ra),
        load_sp = const offset_of!(RiscvRegfile, sp),
        load_gp = const offset_of!(RiscvRegfile, gp),
        load_tp = const offset_of!(RiscvRegfile, tp),
        load_t0 = const offset_of!(RiscvRegfile, t0),
        load_t1 = const offset_of!(RiscvRegfile, t1),
        load_t2 = const offset_of!(RiscvRegfile, t2),
        load_s0 = const offset_of!(RiscvRegfile, s0),
        load_s1 = const offset_of!(RiscvRegfile, s1),
        load_a0 = const offset_of!(RiscvRegfile, a0),
        load_a2 = const offset_of!(RiscvRegfile, a2),
        load_a3 = const offset_of!(RiscvRegfile, a3),
        load_a4 = const offset_of!(RiscvRegfile, a4),
        load_a5 = const offset_of!(RiscvRegfile, a5),
        load_a6 = const offset_of!(RiscvRegfile, a6),
        load_a7 = const offset_of!(RiscvRegfile, a7),
        load_s2 = const offset_of!(RiscvRegfile, s2),
        load_s3 = const offset_of!(RiscvRegfile, s3),
        load_s4 = const offset_of!(RiscvRegfile, s4),
        load_s5 = const offset_of!(RiscvRegfile, s5),
        load_s6 = const offset_of!(RiscvRegfile, s6),
        load_s7 = const offset_of!(RiscvRegfile, s7),
        load_s8 = const offset_of!(RiscvRegfile, s8),
        load_s9 = const offset_of!(RiscvRegfile, s9),
        load_s10 = const offset_of!(RiscvRegfile, s10),
        load_s11 = const offset_of!(RiscvRegfile, s11),
        load_t3 = const offset_of!(RiscvRegfile, t3),
        load_t4 = const offset_of!(RiscvRegfile, t4),
        load_t5 = const offset_of!(RiscvRegfile, t5),
        load_t6 = const offset_of!(RiscvRegfile, t6),
        load_a1 = const offset_of!(RiscvRegfile, a1),
    );
}

impl ArchUsermode for Riscv {
    type KernelRegs = RiscvSavedRegs;
    type UserRegs = RiscvRegfile;

    fn enter_signal(
        frame: &RiscvExceptFrame,
        siginfo: siginfo_t,
        handler: *const (),
        returner: *const (),
    ) -> AccessResult<()> {
        Err(Errno::ENOSYS)
    }

    fn exit_signal(frame: &RiscvExceptFrame) -> AccessResult<()> {
        Err(Errno::ENOSYS)
    }

    unsafe extern "C" fn enter_usermode(load: &UserRegs) {
        unsafe {
            // Disable interrupts, set them to be enabled upon `sret`.
            // Set SPP to 0 to go to U-mode.
            asm!("csrw sstatus, {}", in(reg) csr::sstatus::SPIE_MASK);

            let cpulocal = Riscv::get_cpulocal();
            let thread = Thread::current();
            let runtime = (*thread).runtime();

            enter_usermode_impl(
                &mut (*cpulocal).arch.irq_stack,
                &mut runtime.irq_stack,
                &mut runtime.uctx,
                load,
            );
            // Interrupts re-enabled by `exit_usermode`.
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn exit_usermode(load: &RiscvSavedRegs) -> ! {
        naked_asm!(
            // Disable interrupts.
            "li t0, 0
            csrw sstatus, t0
            ld ra, {save_pc}(a0)",
            // `gp` and `tp` are already loaded.
            "ld sp, {save_sp}(a0)
            ld s0, {save_s0}(a0)
            ld s1, {save_s1}(a0)
            ld s2, {save_s2}(a0)
            ld s3, {save_s3}(a0)
            ld s4, {save_s4}(a0)
            ld s5, {save_s5}(a0)
            ld s6, {save_s6}(a0)
            ld s7, {save_s7}(a0)
            ld s8, {save_s8}(a0)
            ld s9, {save_s9}(a0)
            ld s10, {save_s10}(a0)
            ld s11, {save_s11}(a0)",
            // Jump back to the kernel code.
            "li t0, {sstatus}
            csrw sstatus, t0
            ret",

            sstatus = const csr::sstatus::SIE_MASK,

            save_pc = const offset_of!(RiscvSavedRegs, pc),
            save_sp = const offset_of!(RiscvSavedRegs, sp),
            save_s0 = const offset_of!(RiscvSavedRegs, s0),
            save_s1 = const offset_of!(RiscvSavedRegs, s1),
            save_s2 = const offset_of!(RiscvSavedRegs, s2),
            save_s3 = const offset_of!(RiscvSavedRegs, s3),
            save_s4 = const offset_of!(RiscvSavedRegs, s4),
            save_s5 = const offset_of!(RiscvSavedRegs, s5),
            save_s6 = const offset_of!(RiscvSavedRegs, s6),
            save_s7 = const offset_of!(RiscvSavedRegs, s7),
            save_s8 = const offset_of!(RiscvSavedRegs, s8),
            save_s9 = const offset_of!(RiscvSavedRegs, s9),
            save_s10 = const offset_of!(RiscvSavedRegs, s10),
            save_s11 = const offset_of!(RiscvSavedRegs, s11),
        );
    }

    #[inline(always)]
    fn fallible_load_u8(ptr: *const u8) -> AccessResult<u8> {
        let res;
        if unsafe { noexc_asm!("lbu {}, 0({})", out(reg)res, in(reg)ptr) } {
            return Err(AccessFault);
        }
        Ok(res)
    }

    #[inline(always)]
    fn fallible_load_usize(ptr: *const usize) -> AccessResult<usize> {
        let res;
        if unsafe { noexc_asm!("ld {}, 0({})", out(reg)res, in(reg)ptr) } {
            return Err(AccessFault);
        }
        Ok(res)
    }

    #[inline(always)]
    fn fallible_store_u8(ptr: *const u8, value: u8) -> AccessResult<()> {
        if unsafe { noexc_asm!("sb {}, 0({})", in(reg)value, in(reg)ptr) } {
            return Err(AccessFault);
        }
        Ok(())
    }

    #[inline(always)]
    fn fallible_store_usize(ptr: *const usize, value: usize) -> AccessResult<()> {
        if unsafe { noexc_asm!("sd {}, 0({})", in(reg)value, in(reg)ptr) } {
            return Err(AccessFault);
        }
        Ok(())
    }
}

impl ArchUserRegs for RiscvRegfile {
    fn new(entry_pc: usize, entry_sp: usize) -> Self {
        let mut tmp = Self::default();
        tmp.pc = entry_pc;
        tmp.sp = entry_sp;
        tmp
    }

    fn fork_from(frame: &RiscvExceptFrame) -> Self {
        frame.regs
    }
}
