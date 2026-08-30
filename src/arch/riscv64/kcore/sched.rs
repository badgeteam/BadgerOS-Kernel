use core::{
    arch::{asm, naked_asm},
    ptr::null,
};

use crate::{
    arch::{
        kcore::{cpulocal::ArchCpuLocal, sched::ArchSched},
        riscv64::Riscv,
    },
    badgelib::irq::IrqGuard,
    kcore::sched::{Scheduler, Thread},
};

impl ArchSched for Riscv {
    type FloatState = ();

    #[inline(never)]
    fn current_thread() -> *const Thread {
        // TODO: Sched re-write is needed to be able to use a load relative to `tp`.
        let _noirq = IrqGuard::new();
        unsafe {
            (*Riscv::get_cpulocal())
                .thread
                .as_deref()
                .map(|x| x as *const Thread)
                .unwrap_or(null())
        }
    }

    fn context_create(stack: &mut [usize], ptr: *mut (), meta: *const ()) -> usize {
        const WORDS: usize = 16;
        let len = stack.len();
        let stack = &mut stack[len - WORDS..];
        stack.fill(0);

        // Entrypoint for trampoline.
        stack[15] = meta as usize;
        stack[14] = ptr as usize;
        // Return address for `context_switch`.
        stack[12] = thread_trampoline_1 as *const fn() as usize;

        WORDS
    }

    #[unsafe(naked)]
    extern "C" fn context_switch(
        sched: *const Scheduler,
        new_stack: *mut (),
        old_stack_out: *mut *mut (),
    ) -> *const Scheduler {
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
            "mv   sp, a1",
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

    #[inline(always)]
    fn pause_hint() {
        unsafe { asm!("pause") };
    }
}

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
