use core::arch::asm;

use crate::{
    arch::{
        except::ArchTrapFrame,
        riscv64::{csr, except::RiscvExceptFrame, lazy::insn::is_float_insn},
    },
    util::log::LogLevel,
    kcore::sched::Thread,
    process::usercopy::UserPtr,
};

pub mod float;
pub mod insn;

// If lazy-initialized state has not yet been, initialize it.
// For example, initializes the float state if and only if a float op is being run.
// Returns true if the instruction should be retried.
pub fn check_lazy_init_state(frame: &mut RiscvExceptFrame) -> bool {
    if frame.scause != csr::scause::IILLEGAL || frame.is_kernel_mode() {
        return false;
    }

    // Fetch the affected instruction.
    let mut insn = 0;
    if frame.stval == 0 {
        // Assume impl doesn't support loading instruction into stval.
        unsafe { asm!("csrs sstatus, {}", in(reg)csr::sstatus::MXR_MASK) };
        let res = try {
            insn = UserPtr::<u16>::new(frame.regs.pc as _)?.read()? as u32;
            if insn & 3 == 3 {
                insn |= (UserPtr::<u16>::new((frame.regs.pc + 2) as _)?.read()? as u32) << 16;
            }
        };
        unsafe { asm!("csrc sstatus, {}", in(reg)csr::sstatus::MXR_MASK) };
        if res.is_err() {
            return false; // Only happens if unmapped concurrently.
        }
    } else {
        insn = frame.stval as u32;
    }

    let state;
    unsafe {
        let ptr = Thread::current();
        assert!(!ptr.is_null());
        state = &mut (*ptr).runtime().arch;
    }

    if is_float_insn(insn) {
        logkf!(LogLevel::Debug, "Lazy-FP enable");
        state.enable(frame)
    } else {
        false
    }
}

// Save all lazy state.
pub fn save_lazy_state(frame: &mut RiscvExceptFrame) {
    let state;
    unsafe {
        let ptr = Thread::current();
        if ptr.is_null() {
            return;
        }
        state = &mut (*ptr).runtime().arch;
    }

    state.save_state(frame);
}

// Load all lazy state.
pub fn load_lazy_state(frame: &mut RiscvExceptFrame) {
    let state;
    unsafe {
        let ptr = Thread::current();
        if ptr.is_null() {
            return;
        }
        state = &mut (*ptr).runtime().arch;
    }

    state.load_state(frame);
}
