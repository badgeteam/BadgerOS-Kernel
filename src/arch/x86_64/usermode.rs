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
        frame: &mut crate::arch::except::TrapFrame,
        siginfo: siginfo_t,
        handler: *const (),
        returner: *const (),
    ) -> AccessResult<()> {
        todo!()
    }

    fn exit_signal(frame: &mut crate::arch::except::SyscallFrame) -> AccessResult<()> {
        todo!()
    }

    unsafe extern "C" fn enter_usermode(load: &crate::arch::usermode::UserRegs) {
        todo!()
    }

    unsafe extern "C" fn exit_usermode(restore: &crate::arch::usermode::KernelRegs) -> ! {
        todo!()
    }
}

#[derive(Clone, Copy, Default)]
pub struct DUMMY {}

impl ArchUserRegs for DUMMY {
    fn new(entry_pc: usize, entry_sp: usize) -> Self {
        todo!()
    }

    fn fork_from(frame: &crate::arch::except::SyscallFrame) -> Self {
        todo!()
    }
}
