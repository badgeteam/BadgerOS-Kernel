use crate::kcore::sync::spinlock::RawSpinlock;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Ring {
    Kernel = 0,
    User = 3,
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SegType {
    Code = 0x1e,
    Data = 0x12,
    Ldt = 0x02,
    Tss = 0x09,
    IrqGate = 0x0e,
}

// GDT index: Kernel code.
pub const KCODE_INDEX: usize = 1;
// GDT index: Kernel data/stack.
pub const KDATA_INDEX: usize = 2;
// GDT index: User code (64-bit).
pub const UCODE_INDEX: usize = 3;
// GDT index: User data/stack (64-bit).
pub const UDATA_INDEX: usize = 4;
// GDT index: Task State Segment.
pub const TSS_INDEX: usize = 5;

pub const fn selector(index: usize, ldt: bool, rpl: Ring) -> u16 {
    assert!(index < 0x2000);
    (index as u16) << 3 | (ldt as u16) << 2 | rpl as u16
}

// Segment selector: Kernel code.
pub const KCODE_SEL: u16 = selector(KCODE_INDEX, false, Ring::Kernel);
// Segment selector: Kernel data/stack.
pub const KDATA_SEL: u16 = selector(KDATA_INDEX, false, Ring::Kernel);
// Segment selector: User code (64-bit).
pub const UCODE_SEL: u16 = selector(UCODE_INDEX, false, Ring::User);
// Segment selector: User data/stack (64-bit).
pub const UDATA_SEL: u16 = selector(UDATA_INDEX, false, Ring::User);
// Segment selector: Task State Segment.
pub const TSS_SEL: u16 = selector(TSS_INDEX, false, Ring::Kernel);

impl GenericDesc {
    pub const DPL_SHIFT: u8 = 2;
    pub const PRESENT: u8 = 1 << 7;

    pub const FLAGS_SOFTWARE: u8 = 1 << 4;
    pub const FLAGS_CODE64: u8 = 1 << 5;
    pub const FLAGS_32BIT: u8 = 1 << 6;
    pub const FLAGS_GRAIN: u8 = 1 << 7;

    pub const fn new(
        addr: u32,
        limit: u32,
        type_: SegType,
        dpl: Ring,
        present: bool,
        f_software: bool,
        f_code64: bool,
        f_32bit: bool,
        f_grain: bool,
    ) -> Self {
        assert!(limit < 0x10_0000);
        Self {
            limit0: limit as u16,
            addr0: addr as u16,
            addr1: (addr >> 16) as u8,
            type_dpl_p: type_ as u8
                + ((dpl as u8) << Self::DPL_SHIFT)
                + present as u8 * Self::PRESENT,
            limit1_flags: (limit >> 16) as u8
                + f_software as u8 * Self::FLAGS_SOFTWARE
                + f_code64 as u8 * Self::FLAGS_CODE64
                + f_32bit as u8 * Self::FLAGS_32BIT
                + f_grain as u8 * Self::FLAGS_GRAIN,
            addr2: (addr >> 24) as u8,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GenericDesc {
    pub limit0: u16,
    pub addr0: u16,
    pub addr1: u8,
    pub type_dpl_p: u8,
    pub limit1_flags: u8,
    pub addr2: u8,
}

impl GateDesc {
    pub const fn new(addr: u32, cs: u16, type_: SegType, dpl: Ring, present: bool) -> Self {
        Self {
            addr0: addr as u16,
            cs,
            _resvd0: 0,
            type_dpl_p: type_ as u8
                + ((dpl as u8) << GenericDesc::DPL_SHIFT)
                + present as u8 * GenericDesc::PRESENT,
            addr1: (addr >> 16) as u16,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GateDesc {
    pub addr0: u16,
    pub cs: u16,
    pub _resvd0: u8,
    pub type_dpl_p: u8,
    pub addr1: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ExtAddrEnt {
    pub addr3: u32,
    pub zero: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union Descriptor {
    pub null: u64,
    pub generic: GenericDesc,
    pub gate: GateDesc,
    pub ext_addr: ExtAddrEnt,
}

#[repr(C, packed(2))]
pub struct DescTableAddr {
    pub limit: u16,
    pub addr: *const Descriptor,
}

#[repr(C, packed(4))]
pub struct Tss {
    pub _resvd0: u32,
    pub rsp: [u64; 4],
    /// Index 0 is reserved.
    pub ist: [u64; 8],
    pub _resvd1: [u32; 2],
    pub _resvd2: u16,
    pub iopb_off: u16,
}

pub static TSS_LOCK: RawSpinlock = RawSpinlock::new();

pub type Gdt = [Descriptor; 7];
pub static mut GDT: Gdt = const {
    let mut entries = [Descriptor { null: 0 }; 7];

    entries[KCODE_INDEX].generic = GenericDesc::new(
        0,
        0,
        SegType::Code,
        Ring::Kernel,
        true,
        false,
        true,
        false,
        false,
    );
    entries[KDATA_INDEX].generic = GenericDesc::new(
        0,
        0,
        SegType::Data,
        Ring::Kernel,
        true,
        false,
        false,
        false,
        false,
    );
    entries[UCODE_INDEX].generic = GenericDesc::new(
        0,
        0,
        SegType::Code,
        Ring::User,
        true,
        false,
        true,
        false,
        false,
    );
    entries[UDATA_INDEX].generic = GenericDesc::new(
        0,
        0,
        SegType::Data,
        Ring::User,
        true,
        false,
        false,
        false,
        false,
    );
    entries[TSS_INDEX].generic = GenericDesc::new(
        0,
        0,
        SegType::Tss,
        Ring::Kernel,
        true,
        false,
        false,
        false,
        false,
    );
    entries[TSS_INDEX + 1].ext_addr = ExtAddrEnt { addr3: 0, zero: 0 };

    entries
};

pub type Idt = [Descriptor; 512];
pub static mut IDT: Idt = const { [Descriptor { null: 0 }; _] };

unsafe extern "C" {
    #[link_name = "x86_idtstubs"]
    static IDT_STUBS: [u64; 256];
}

pub unsafe fn construct_idt() {
    unsafe {
        for i in 0..256 {
            IDT[i * 2].gate = GateDesc::new(
                IDT_STUBS[i] as u32,
                KCODE_SEL,
                SegType::IrqGate,
                Ring::Kernel,
                true,
            );
            IDT[i * 2 + 1].ext_addr = ExtAddrEnt {
                addr3: (IDT_STUBS[i] >> 32) as u32,
                zero: 0,
            }
        }
    }
}
