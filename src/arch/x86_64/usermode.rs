use crate::{
    arch::{
        usermode::{ArchUserRegs, ArchUsermode},
        x86_64::X86_64,
    },
    process::{uapi::signal::siginfo_t, usercopy::AccessResult},
};

impl ArchUsermode for X86_64 {
    type KernelRegs = ();

    type UserRegs = DUMMY;

    fn enter_signal(
        _frame: &mut crate::arch::except::TrapFrame,
        _siginfo: siginfo_t,
        _handler: *const (),
        _returner: *const (),
    ) -> AccessResult<()> {
        todo!()
    }

    fn exit_signal(_frame: &mut crate::arch::except::SyscallFrame) -> AccessResult<()> {
        todo!()
    }

    unsafe extern "C" fn enter_usermode(_load: &crate::arch::usermode::UserRegs) {
        todo!()
    }

    unsafe extern "C" fn exit_usermode(_restore: &crate::arch::usermode::KernelRegs) -> ! {
        todo!()
    }
}

#[derive(Clone, Copy, Default)]
pub struct DUMMY {}

impl ArchUserRegs for DUMMY {
    fn new(_entry_pc: usize, _entry_sp: usize) -> Self {
        todo!()
    }

    fn fork_from(_frame: &crate::arch::except::SyscallFrame) -> Self {
        todo!()
    }
}
