use core::arch::naked_asm;

use crate::arch::{misc::ArchMisc, riscv64::Riscv};

impl ArchMisc for Riscv {
    const FP_RA_OFFSET: isize = -8;
    const FP_LINK_OFFSET: isize = -16;

    #[unsafe(naked)]
    extern "C" fn cur_frame_ptr() -> *const () {
        naked_asm!("mv a0, fp", "ret");
    }
}
