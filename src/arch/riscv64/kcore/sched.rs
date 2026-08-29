use crate::arch::{kcore::sched::ArchSched, riscv64::Riscv};

impl ArchSched for Riscv {
    type FloatState = ();

    fn current_thread() -> *const crate::kcore::sched::Thread {
        todo!()
    }

    fn context_create(stack: &[usize], ptr: *mut (), meta: *const ()) -> usize {
        todo!()
    }

    fn context_switch(
        sched: *const crate::kcore::sched::Scheduler,
        new_stack: *mut (),
        old_stack_out: *mut *mut (),
    ) -> *const crate::kcore::sched::Scheduler {
        todo!()
    }
}
