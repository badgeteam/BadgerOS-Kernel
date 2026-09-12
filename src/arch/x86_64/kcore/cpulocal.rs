use core::{arch::asm, mem::offset_of};

use crate::{
    arch::{
        kcore::cpulocal::{ArchCpuLocal, ArchCpuLocalData},
        x86_64::{X86_64, msr, seg},
    },
    kcore::cpulocal::CpuLocal,
};

impl ArchCpuLocal for X86_64 {
    type CpuLocalData = X86CpuLocalData;

    fn get_cpulocal() -> *mut CpuLocal {
        let ptr;
        unsafe {
            asm!("mov {ptr}, gs:[{off}]",
            ptr = out(reg)ptr,
            off = const offset_of!(CpuLocal, arch)
                + offset_of!(X86CpuLocalData, self_ptr)
            );
        }
        ptr
    }

    unsafe fn set_cpulocal(next: *mut CpuLocal) {
        unsafe {
            (*next).arch.self_ptr = next;
            msr::write(msr::gsbase::ADDR, next as _);
            msr::write(msr::kgsbase::ADDR, next as _);
        }
    }
}

pub struct X86CpuLocalData {
    pub self_ptr: *const CpuLocal,
    pub tss: seg::Tss,
}

impl Default for X86CpuLocalData {
    fn default() -> Self {
        Self {
            self_ptr: Default::default(),
            tss: seg::Tss {
                _resvd0: 0,
                rsp: [0; _],
                ist: [0; _],
                _resvd1: [0; _],
                _resvd2: 0,
                iopb_off: size_of::<seg::Tss>() as _,
            },
        }
    }
}

impl ArchCpuLocalData for X86CpuLocalData {
    fn set_irq_stack(&mut self, sp: *mut ()) {
        self.tss.rsp[0] = sp as _;
    }
}
