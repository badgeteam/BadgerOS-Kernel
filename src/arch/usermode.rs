use crate::process::usercopy::AccessResult;

/// User-mode management trait.
pub trait ArchUsermode {
    /// Where [`ArchUsermode::enter_usermode`] stores the kernel registers.
    type KernelRegs: Sized + Copy;

    /// Enter usermode given a prepared PC and stack.
    fn enter_usermode(u_pc: usize, u_sp: usize);
    /// Exit usermode by restoring the kernel register state.
    fn exit_usermode();

    /// Load byte, check for access faults instead of panicking.
    fn fallible_load_u8(ptr: *const u8) -> AccessResult<u8>;
    /// Load usize, check for access faults instead of panicking.
    fn fallible_load_usize(ptr: *const usize) -> AccessResult<usize>;
    /// Store byte, check for access faults instead of panicking.
    fn fallible_store_u8(ptr: *const u8, value: u8) -> AccessResult<()>;
    /// Store usize, check for access faults instead of panicking.
    fn fallible_store_usize(ptr: *const usize, value: usize) -> AccessResult<()>;
}
