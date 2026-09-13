use core::arch::asm;

use limine::mp::MpInfo;

use crate::{
    arch::{
        kcore::{cpulocal::ArchCpuLocal, smp::ArchSmp},
        x86_64::{
            X86_64,
            msr::{self, gsbase},
            seg::*,
        },
    },
    kcore::smp::limine_trampoline_2,
};

impl ArchSmp for X86_64 {
    type CpuID = u16;

    fn cpu_spinup() {
        unsafe {
            let cpulocal = X86_64::get_cpulocal();
            let _guard = TSS_LOCK.lock();

            let tss_addr = &raw const (*cpulocal).arch.tss as u64;
            GDT[TSS_INDEX].generic = GenericDesc::new(
                tss_addr as u32,
                size_of::<Tss>() as u32,
                SegType::Tss,
                Ring::Kernel,
                true,
                false,
                false,
                false,
                false,
            );
            GDT[TSS_INDEX + 1].ext_addr = ExtAddrEnt {
                addr3: (tss_addr >> 32) as u32,
                zero: 0,
            };

            let gdtr = DescTableAddr {
                limit: size_of::<Gdt>() as u16 - 1,
                addr: &raw const GDT[0],
            };
            let idtr = DescTableAddr {
                limit: size_of::<Idt>() as u16 - 1,
                addr: &raw const IDT[0],
            };

            asm! {
                "
                lgdt [{gdtr}]
                lidt [{idtr}]
                
                mov ax, {kdata}
                mov ds, ax
                mov es, ax
                mov fs, ax
                mov gs, ax
                mov ss, ax
                
                push {kcode}
                lea rax, [2f+rip]
                push rax
                retfq
            2:
                mov ax, {tss}
                ltr ax
                ",
                gdtr = in(reg)&gdtr,
                idtr = in(reg)&idtr,
                kdata = const KDATA_SEL,
                kcode = const KCODE_SEL,
                tss = const TSS_SEL,
                out("rax") _,
            }
            // Reloading `gs` will have destroyed the `GSBASE` MSR; reload it.
            msr::write(gsbase::ADDR, cpulocal as u64);
        }
    }

    unsafe extern "C" fn limine_trampoline_1(info: &MpInfo) -> ! {
        unsafe { limine_trampoline_2(info) };
    }
}
