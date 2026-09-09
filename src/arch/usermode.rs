use crate::process::{uapi::signal::siginfo_t, usercopy::AccessResult};

use super::{
    Arch,
    except::{SyscallFrame, TrapFrame},
};

/// User-mode management trait.
pub trait ArchUsermode {
    /// Where [`ArchUsermode::enter_usermode`] stores the kernel registers.
    type KernelRegs: Default + Sized + Copy + Send;
    /// User register file initializer.
    type UserRegs: ArchUserRegs;

    /// Enter userspace signal handler.
    fn enter_signal(
        frame: &TrapFrame,
        siginfo: siginfo_t,
        handler: *const (),
        returner: *const (),
    ) -> AccessResult<()>;
    /// Exit userspace signal handler.
    fn exit_signal(frame: &SyscallFrame) -> AccessResult<()>;
    /// Enter usermode given a prepared PC and stack.
    unsafe extern "C" fn enter_usermode(load: &UserRegs);
    /// Exit usermode by restoring the kernel register state.
    unsafe extern "C" fn exit_usermode(restore: &KernelRegs) -> !;
}

pub trait ArchUserRegs: Default + Sized + Copy + Send {
    /// Create register state for a new user thread.
    fn new(entry_pc: usize, entry_sp: usize) -> Self;
    /// Create a copy of register state for the `fork` syscall.
    fn fork_from(frame: &SyscallFrame) -> Self;
}

/// Where [`ArchUsermode::enter_usermode`] stores the kernel registers.
pub type KernelRegs = <Arch as ArchUsermode>::KernelRegs;
/// User register file initializer.
pub type UserRegs = <Arch as ArchUsermode>::UserRegs;
