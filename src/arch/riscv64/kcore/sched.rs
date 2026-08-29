use core::{arch::asm, mem::offset_of};

use crate::{
    arch::{kcore::sched::ArchSched, riscv64::Riscv},
    kcore::{
        cpulocal::CpuLocal,
        sched::{Scheduler, Thread},
    },
};

impl ArchSched for Riscv {
    type FloatState = ();

    fn cpu_spinup() {
        unsafe {
            asm!("csrw sstatus, 0");
            // asm!("csrw stvec, {}", in(reg) riscv_vector_table as *const () as usize);
            asm!("csrw sie, {}", in(reg)(1 << 9)); // Supervisor external interrupt.
        }
    }

    #[inline(always)]
    fn current_thread() -> *const Thread {
        unsafe {
            let raw: *const Thread;
            asm!("ld {}, {}(tp)", out(reg)raw, const offset_of!(CpuLocal, thread));
            if raw.is_null() {
                return raw;
            }
            // Offset of the struct `Thread` inside the `ArcInner`.
            // TODO: Find an alternative solution that doesn't assume such an implementation detail.
            raw.byte_add(16)
        }
    }

    fn context_create(stack: &[usize], ptr: *mut (), meta: *const ()) -> usize {
        todo!()
    }

    extern "C" fn context_switch(
        sched: *const Scheduler,
        new_stack: *mut (),
        old_stack_out: *mut *mut (),
    ) -> *const Scheduler {
        todo!()
    }

    #[inline(always)]
    fn pause_hint() {
        unsafe { asm!("pause") };
    }
}
