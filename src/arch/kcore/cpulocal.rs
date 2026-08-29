use crate::kcore::cpulocal::CpuLocal;

/// CPU-local data trait.
pub trait ArchCpuLocal {
    /// Architecture-specific CPU-local data.
    type CpuLocalData: ArchCpuLocalData;

    /// Get the CPU-local pointer.
    fn get_cpulocal() -> *mut CpuLocal;

    /// Set the CPU-local pointer.
    unsafe fn set_cpulocal(next: *mut CpuLocal);
}

pub trait ArchCpuLocalData: Default + Sized {
    /// Set the stack pointer used for traps and interrupts.
    fn set_irq_stack(&mut self, sp: *mut ());
}
