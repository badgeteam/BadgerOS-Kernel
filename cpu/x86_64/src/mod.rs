use bytemuck_derive::{AnyBitPattern, NoUninit};
use core::arch::asm;

pub mod backtrace;
pub mod cpulocal;
mod gdt;
mod ioport;
pub mod irq;
pub mod mmu;
mod msr;
pub mod panic;
pub mod spinup;
pub mod thread;
pub mod timer;
pub mod usercopy;
pub mod usermode;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, AnyBitPattern, NoUninit)]
struct CpuID {
    eax: u32,
    ebx: u32,
    ecx: u32,
    edx: u32,
}

#[inline(always)]
fn cpuid(index: u32) -> Option<CpuID> {
    if index != 0 {
        unsafe {
            let max: u32;
            asm!(
                "mov {0:r}, rbx",
                "cpuid",
                "mov rbx, {0:r}",
                out(reg) _,
                inout("eax") index => max,
                out("ecx") _,
                out("edx") _,
                options(nostack, preserves_flags),
            );
            if max < index {
                return None;
            }
        }
    }

    let mut out = CpuID::default();
    unsafe {
        asm!(
            "mov {0:r}, rbx",
            "cpuid",
            "xchg {0:r}, rbx",
            out(reg) out.ebx,
            inout("eax") index => out.eax,
            out("ecx") out.ecx,
            out("edx") out.edx,
            options(nostack, preserves_flags),
        );
    }

    Some(out)
}

pub type PhysCpuID = u16;

/// Detectable features that BadgerOS can run without but needs to support for userspace to use it.
#[derive(Default, Clone, Copy)]
pub struct CpuFeatures {}

pub const MACHINE_NAME: &'static str = "x86_64";
