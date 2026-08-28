use crate::kcore::cpulocal::CpuLocal;

/// CPU-local data trait.
pub trait ArchCpuLocal {
    /// Architecture-specific CPU-local data.
    type ExtraData: Sized;

    /// Get the CPU-local pointer.
    fn get() -> *mut CpuLocal;

    /// Set the CPU-local pointer.
    unsafe fn set(next: *mut CpuLocal);
}
