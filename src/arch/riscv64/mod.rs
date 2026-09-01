use super::ArchTrait;

pub struct Riscv;

pub mod csr;
pub mod except;
pub mod kcore;
pub mod misc;
pub mod mmu;
pub mod sbi;
pub mod usermode;

impl ArchTrait for Riscv {
    const MACHINE: &'static str = "riscv64";
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RiscvSavedRegs {
    pub pc: usize,
    // ra not saved as it is redundant with `pc`
    pub sp: usize,
    // gp not saved as it always contains the same value
    // tp not saved as it is used for the cpulocal ptr
    pub s0: usize,
    pub s1: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RiscvRegfile {
    pub pc: usize,
    pub ra: usize,
    pub sp: usize,
    pub gp: usize,
    pub tp: usize,
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,
    pub s0: usize,
    pub s1: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize,
}
