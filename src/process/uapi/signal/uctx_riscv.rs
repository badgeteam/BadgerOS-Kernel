// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::process::{uapi::sigset::sigset_t, usercopy::UserCopyable};

use super::stack_t;

pub const NREG: usize = 32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct __riscv_f_ext_state {
    pub f: [u32; 32],
    pub fcsr: u32,
}
unsafe impl UserCopyable for __riscv_f_ext_state {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct __riscv_d_ext_state {
    pub f: [u64; 32],
    pub fcsr: u32,
}
unsafe impl UserCopyable for __riscv_d_ext_state {}

#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct __riscv_q_ext_state {
    pub f: [u64; 64],
    pub fcsr: u32,
    pub _resvd0: [u32; 3],
}
unsafe impl UserCopyable for __riscv_q_ext_state {}

#[repr(C)]
#[derive(Clone, Copy)]
pub union __riscv_fp_state {
    pub f: __riscv_f_ext_state,
    pub d: __riscv_d_ext_state,
    pub q: __riscv_q_ext_state,
}
unsafe impl UserCopyable for __riscv_fp_state {}
impl Default for __riscv_fp_state {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

pub type __riscv_mc_gp_state = [usize; NREG];

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct sigcontext {
    pub gregs: __riscv_mc_gp_state,
    pub fpregs: __riscv_fp_state,
}
unsafe impl UserCopyable for sigcontext {}
pub type mcontext_t = sigcontext;

#[repr(C)]
#[derive(Clone, Copy)]
pub union __uc_sigmask_union {
    pub uc_sigmask: sigset_t,
    pub unused: [u8; 1024 / 8],
}
unsafe impl UserCopyable for __uc_sigmask_union {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ucontext_t {
    pub uc_flags: usize,
    pub uc_link: *mut ucontext_t,
    pub uc_stack: stack_t,
    pub __uc_sigmask_union: __uc_sigmask_union,
    pub uc_mcontext: mcontext_t,
}
unsafe impl UserCopyable for ucontext_t {}
