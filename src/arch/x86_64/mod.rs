use crate::{arch::ArchTrait, boot::init::basic_runtime_init};

pub mod cpuid;
pub mod except;
pub mod kcore;
pub mod misc;
pub mod mmu;
pub mod msr;
pub mod seg;
pub mod usermode;

pub struct X86_64;

impl ArchTrait for X86_64 {
    const MACHINE: &'static str = "x86_64";
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    unsafe {
        cpuid::cache_leaf_0();
        seg::construct_idt();
        basic_runtime_init();
    }
}
