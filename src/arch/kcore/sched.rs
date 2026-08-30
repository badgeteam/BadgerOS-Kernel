use crate::{
    arch::Arch,
    kcore::sched::{Scheduler, Thread},
};

/// Scheduler context trait.
pub trait ArchSched {
    /// Floating-point save-state.
    type FloatState: Default + Sized + Copy;

    /// Get a pointer to the currently running thread, if any.
    fn current_thread() -> *const Thread;

    /// Prepare a new thread context to jump to [`Thread::thread_trampoline_2`]
    /// Returns how much stack was used in words.
    fn context_create(stack: &[usize], ptr: *mut (), meta: *const ()) -> usize;

    /// Switch between thread contexts.
    /// Passes thru `sched` so that the new thread knows what CPU it's on without checking the CPU-local data.
    extern "C" fn context_switch(
        sched: *const Scheduler,
        new_stack: *mut (),
        old_stack_out: *mut *mut (),
    ) -> *const Scheduler;

    /// CPU pause hint, commonly used for spinning on locks.
    fn pause_hint() {}
}

pub type FloatState = <Arch as ArchSched>::FloatState;
