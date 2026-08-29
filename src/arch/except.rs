use core::fmt::Display;

use super::Arch;

/// Exception handling trait.
pub trait ArchExcept {
    /// System call frame.
    type SyscallFrame: ArchSyscallFrame;
    /// Trap frame.
    type TrapFrame: ArchTrapFrame;

    /// Enable interrupts.
    fn enable_irq();
    /// Disable interrupts.
    fn disable_irq();
    /// Query whether interrupts are enabled.
    fn get_irq_enabled() -> bool;
    /// Conditionally enable interrupts.
    fn enable_irq_if(cond: bool) {
        if cond {
            Self::enable_irq();
        }
    }
    /// Disable interrupts and return whether they were enabled.
    fn get_disable_irq() -> bool {
        let get = Self::get_irq_enabled();
        Self::disable_irq();
        get
    }
}

/// System call frame.
pub type SyscallFrame = <Arch as ArchExcept>::SyscallFrame;
/// Trap frame.
pub type TrapFrame = <Arch as ArchExcept>::TrapFrame;

/// System call frame trait.
pub const trait ArchSyscallFrame {
    /// Set system call return value.
    fn set_retval(&mut self, value: usize);
}

/// Trap frame trait.
pub const trait ArchTrapFrame: Display {
    /// Trap is from kernel mode.
    fn is_kernel_mode(&self) -> bool;
    /// Trap cause.
    fn get_cause(&self) -> Option<TrapCause>;
    /// Trap name.
    fn get_name(&self) -> Option<&str>;
    /// Trap number.
    fn get_number(&self) -> usize;
    /// Trapping address for access faults.
    fn get_addr(&self) -> Option<usize>;
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
