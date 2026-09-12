use core::arch::asm;

#[derive(Clone, Copy)]
pub struct Cpuid {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
}

static mut MAX_EXT_LEAF: u32 = 0;
static mut MAX_LEAF: u32 = 0;
static mut VENDOR: [u8; 12] = [0; 12];

pub unsafe fn cache_leaf_0() {
    unsafe {
        let res = cpuid(0);
        MAX_LEAF = res.eax;
        VENDOR[0..4].copy_from_slice(&res.ebx.to_le_bytes());
        VENDOR[4..8].copy_from_slice(&res.edx.to_le_bytes());
        VENDOR[8..12].copy_from_slice(&res.ecx.to_le_bytes());

        MAX_EXT_LEAF = cpuid(0x8000_0000).eax;
    }
}

pub fn cpuid(leaf: u32) -> Cpuid {
    unsafe {
        let mut res = Cpuid {
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
        };
        if (leaf >> 16) == 0x0000 && (leaf & 0xffff) <= MAX_LEAF
            || (leaf >> 16) == 0x8000 && (leaf & 0xffff) <= MAX_EXT_LEAF
        {
            asm!(
                "
                mov {ebx:r}, rbx
                cpuid
                xchg {ebx:r}, rbx
                ",
                inout("rax")leaf => res.eax,
                ebx=out(reg)res.ebx,
                out("rcx")res.ecx,
                out("rdx")res.edx,
                options(nomem, pure, nostack, preserves_flags)
            );
        }
        res
    }
}
