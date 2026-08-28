/// User-mode management trait.
pub trait ArchUsermode {
    /// Where [`ArchUsermode::enter_usermode`] stores the kernel registers.
    type KernelRegs: Sized + Copy;

    /// Enter usermode given a prepared PC and stack.
    fn enter_usermode(u_pc: usize, u_sp: usize);

    /// Exit usermode by restoring the kernel register state.
    fn exit_usermode();
}
