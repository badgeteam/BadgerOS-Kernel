use core::{arch::naked_asm, ptr::null};

use crate::{
    arch::{
        kcore::{cpulocal::ArchCpuLocal, sched::ArchSched},
        x86_64::X86_64,
    },
    badgelib::irq::IrqGuard,
    kcore::sched::{Scheduler, Thread},
};

impl ArchSched for X86_64 {
    type ThreadArchState = ();

    fn current_thread() -> *const Thread {
        // TODO: Sched re-write is needed to be able to use a load relative to `gs`.
        let _noirq = IrqGuard::new();
        unsafe {
            (*X86_64::get_cpulocal())
                .thread
                .as_deref()
                .map(|x| x as *const Thread)
                .unwrap_or(null())
        }
    }

    fn context_create(stack: &mut [usize], ptr: *mut (), meta: *const ()) -> usize {
        const WORDS: usize = 9;
        let len = stack.len();
        let stack = &mut stack[len - WORDS..];
        stack.fill(0);

        // Entrypoint for trampoline.
        stack[8] = meta as usize;
        stack[7] = ptr as usize;
        // Return address for `context_switch`.
        stack[6] = thread_trampoline_1 as *const fn() as usize;

        WORDS
    }

    #[unsafe(naked)]
    extern "C" fn context_switch(
        sched: *const Scheduler,
        new_stack: *const *mut (),
        old_stack_out: *mut *mut (),
    ) -> *const Scheduler {
        naked_asm!(
            "push rbx
            push rbp
            push r12
            push r13
            push r14
            push r15
            
            mov rax, rdi
            mov [rsi], rsp
            mov rsp, [rdx]
            
            pop r15
            pop r14
            pop r13
            pop r12
            pop rbp
            pop rbx
            ret"
        )
    }
}

/// Part 1: Load the raw parts of the `Box<dyn FnOnce()>`.
#[unsafe(naked)]
pub unsafe extern "C" fn thread_trampoline_1() {
    naked_asm!(
        "pop rdi
        pop rsi
        jmp  {}",
        sym Thread::thread_trampoline_2
    );
}
