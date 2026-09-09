use core::fmt::Display;

use crate::process::usercopy::AccessResult;

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

    /// Load byte, check for access faults instead of panicking.
    fn fallible_load_u8(ptr: *const u8) -> AccessResult<u8>;
    /// Load two-byte, check for access faults instead of panicking.
    fn fallible_load_u16(ptr: *const u16) -> AccessResult<u16>;
    /// Load four-byte, check for access faults instead of panicking.
    fn fallible_load_u32(ptr: *const u32) -> AccessResult<u32>;
    /// Load eight-byte, check for access faults instead of panicking.
    fn fallible_load_u64(ptr: *const u64) -> AccessResult<u64>;
    /// Load usize, check for access faults instead of panicking.
    fn fallible_load_usize(ptr: *const usize) -> AccessResult<usize>;
    /// Store byte, check for access faults instead of panicking.
    fn fallible_store_u8(ptr: *const u8, value: u8) -> AccessResult<()>;
    /// Store two-byte, check for access faults instead of panicking.
    fn fallible_store_u16(ptr: *const u16, value: u16) -> AccessResult<()>;
    /// Store four-byte, check for access faults instead of panicking.
    fn fallible_store_u32(ptr: *const u32, value: u32) -> AccessResult<()>;
    /// Store eight-byte, check for access faults instead of panicking.
    fn fallible_store_u64(ptr: *const u64, value: u64) -> AccessResult<()>;
    /// Store usize, check for access faults instead of panicking.
    fn fallible_store_usize(ptr: *const usize, value: usize) -> AccessResult<()>;
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
    /// Handle `.noexc_table` hit.
    fn noexc_skip(&mut self, addr: *const ());
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
