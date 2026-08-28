/// Exception handling trait.
pub trait ArchExcept {
    /// System call frame.
    type SyscallFrame: ArchSyscallFrame;
    /// Trap frame.
    type TrapFrame: ArchTrapFrame;

    /// Get frame pointer for backtraces.
    fn caller_frame_ptr() -> *const ();

    /// Enable interrupts.
    fn enable_irq();
    /// Disable interrupts.
    fn disable_irq();
    /// Query whether interrupts are enabled.
    fn get_irq() -> bool;
    /// Conditionally enable interrupts.
    fn enable_irq_if(cond: bool) {
        if cond {
            Self::enable_irq();
        }
    }
    /// Disable interrupts and return whether they were enabled.
    fn get_disable_irq() -> bool {
        let get = Self::get_irq();
        Self::disable_irq();
        get
    }
}

/// System call frame trait.
pub const trait ArchSyscallFrame {
    /// Set system call return value.
    fn set_retval(&mut self, value: usize);
}

/// Trap frame trait.
pub const trait ArchTrapFrame {
    /// Trap is from kernel mode.
    fn is_kernel_mode(&self) -> bool;
    /// Trap cause.
    fn get_cause(&self) -> TrapCause;
    /// Trapping instruction address.
    fn get_pc(&self) -> *const ();
    /// Backtrace frame pointer; null if not available.
    fn get_frame_ptr(&self) -> *const ();
}

/// Trap causes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TrapCause {
    /// Illegal instruction fault.
    IllegalInsn,
    /// Hardware/software breakpoint hit.
    Breakpoint,
    /// Invalid arithmetic (e.g. division by zero).
    ArithmeticFault,
    /// Page fault (load).
    PageFaultLoad,
    /// Page fault (store).
    PageFaultStore,
    /// Page fault (execute).
    PageFaultExec,
}
