use core::arch::naked_asm;

use crate::arch::{misc::ArchMisc, x86_64::X86_64};

impl ArchMisc for X86_64 {
    const FP_RA_OFFSET: isize = 8;

    const FP_LINK_OFFSET: isize = 0;

    #[unsafe(naked)]
    extern "C" fn cur_frame_ptr() -> *const () {
        naked_asm!(
            "mov rax, rbp
            ret"
        );
    }
}
