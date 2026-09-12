use core::{
    arch::{asm, global_asm},
    fmt::Display,
    mem::offset_of,
};

use crate::{
    arch::{
        except::{ArchExcept, ArchSyscallFrame, ArchTrapFrame, TrapCause},
        x86_64::{X86_64, seg::KCODE_SEL},
    },
    except::generic_trap,
    process::usercopy::{AccessFault, AccessResult},
};

global_asm!(
    include_str!("except.S"),

    X86TrapFrame_rip = const offset_of!(X86TrapFrame, rip),
    X86TrapFrame_rsp = const offset_of!(X86TrapFrame, rsp),

    X86TrapFrame_r15 = const offset_of!(X86TrapFrame, r15),
    X86TrapFrame_r14 = const offset_of!(X86TrapFrame, r14),
    X86TrapFrame_r13 = const offset_of!(X86TrapFrame, r13),
    X86TrapFrame_r12 = const offset_of!(X86TrapFrame, r12),
    X86TrapFrame_r11 = const offset_of!(X86TrapFrame, r11),
    X86TrapFrame_r10 = const offset_of!(X86TrapFrame, r10),
    X86TrapFrame_r9 = const offset_of!(X86TrapFrame, r9),
    X86TrapFrame_r8 = const offset_of!(X86TrapFrame, r8),
    X86TrapFrame_rdi = const offset_of!(X86TrapFrame, rdi),
    X86TrapFrame_rsi = const offset_of!(X86TrapFrame, rsi),
    X86TrapFrame_rbp = const offset_of!(X86TrapFrame, rbp),
    X86TrapFrame_rdx = const offset_of!(X86TrapFrame, rdx),
    X86TrapFrame_rcx = const offset_of!(X86TrapFrame, rcx),
    X86TrapFrame_rbx = const offset_of!(X86TrapFrame, rbx),
    X86TrapFrame_rax = const offset_of!(X86TrapFrame, rax),
);

impl ArchExcept for X86_64 {
    type SyscallFrame = DUMMY;

    type TrapFrame = X86TrapFrame;

    fn enable_irq() {
        unsafe { asm!("cli") }
    }

    fn disable_irq() {
        unsafe { asm!("sti") }
    }

    fn get_irq_enabled() -> bool {
        let rflags: u64;
        unsafe { asm!("pushf; pop {}", out(reg)rflags) };
        rflags & (1 << 9) != 0
    }

    fn fallible_load_u8(ptr: *const u8) -> AccessResult<u8> {
        Err(AccessFault)
    }

    fn fallible_load_u16(ptr: *const u16) -> AccessResult<u16> {
        Err(AccessFault)
    }

    fn fallible_load_u32(ptr: *const u32) -> AccessResult<u32> {
        Err(AccessFault)
    }

    fn fallible_load_u64(ptr: *const u64) -> AccessResult<u64> {
        Err(AccessFault)
    }

    fn fallible_load_usize(ptr: *const usize) -> AccessResult<usize> {
        Err(AccessFault)
    }

    fn fallible_store_u8(ptr: *const u8, value: u8) -> AccessResult<()> {
        Err(AccessFault)
    }

    fn fallible_store_u16(ptr: *const u16, value: u16) -> AccessResult<()> {
        Err(AccessFault)
    }

    fn fallible_store_u32(ptr: *const u32, value: u32) -> AccessResult<()> {
        Err(AccessFault)
    }

    fn fallible_store_u64(ptr: *const u64, value: u64) -> AccessResult<()> {
        Err(AccessFault)
    }

    fn fallible_store_usize(ptr: *const usize, value: usize) -> AccessResult<()> {
        Err(AccessFault)
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct X86TrapFrame {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rbp: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,

    pub irq: u64,
    pub code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl Display for X86TrapFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "  RIP  0x{:016x}\n", self.rip)?;

        write!(
            f,
            "  RAX  0x{:016x}  RBX  0x{:016x}  RCX  0x{:016x}  RDX  0x{:016x}\n",
            self.rax, self.rbx, self.rcx, self.rdx,
        )?;
        write!(
            f,
            "  RDI  0x{:016x}  RSI  0x{:016x}  RBP  0x{:016x}  RSP  0x{:016x}\n",
            self.rdi, self.rsi, self.rbp, self.rsp,
        )?;
        write!(
            f,
            "  R8   0x{:016x}  R9   0x{:016x}  R10  0x{:016x}  R11  0x{:016x}\n",
            self.r8, self.r9, self.r10, self.r11,
        )?;
        write!(
            f,
            "  R12  0x{:016x}  R13  0x{:016x}  R14  0x{:016x}  R15  0x{:016x}\n",
            self.r12, self.r13, self.r14, self.r15,
        )?;

        write!(f, "  RFLAGS  0x{:016x}\n", self.rflags)?;
        write!(f, "  ERR#    0x{:08x}\n", self.code)?;
        write!(f, "  CS      0x{:04x}\n", self.cs)?;
        write!(f, "  SS      0x{:04x}\n", self.ss)?;
        Ok(())
    }
}

impl ArchTrapFrame for X86TrapFrame {
    fn is_kernel_mode(&self) -> bool {
        self.cs as u16 == KCODE_SEL
    }

    fn get_cause(&self) -> Option<TrapCause> {
        None
    }

    fn get_name(&self) -> Option<&str> {
        match self.irq {
            0 => Some("#DE"),
            1 => Some("#DB"),
            2 => Some("#NMI"),
            3 => Some("#BP"),
            4 => Some("#OF"),
            5 => Some("#BR"),
            6 => Some("#UD"),
            7 => Some("#NM"),
            8 => Some("#DF"),
            9 => Some("Coprocessor segment overrun"),
            10 => Some("#TS"),
            11 => Some("#NP"),
            12 => Some("#SS"),
            13 => Some("#GP"),
            14 => Some("#PF"),
            16 => Some("#MF"),
            17 => Some("#AC"),
            18 => Some("#MC"),
            19 => Some("#XF"),
            21 => Some("#CP"),
            28 => Some("#HV"),
            29 => Some("#VC"),
            30 => Some("#SX"),
            _ => None,
        }
    }

    fn get_number(&self) -> usize {
        self.irq as _
    }

    fn get_addr(&self) -> Option<usize> {
        None
    }

    fn get_pc(&self) -> *const () {
        self.rip as _
    }

    fn noexc_skip(&mut self, addr: *const ()) {
        todo!()
    }

    fn get_frame_ptr(&self) -> *const () {
        0 as _
    }
}

#[derive(Clone, Copy)]
pub struct DUMMY {}

impl Display for DUMMY {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        todo!()
    }
}

impl ArchSyscallFrame for DUMMY {
    fn set_retval(&mut self, value: usize) {
        todo!()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_irq_handler(frame: &mut X86TrapFrame) {
    generic_trap(frame);
}
