use crate::{
    arch::usermode::ArchUsermode,
    process::usercopy::{AccessFault, AccessResult},
};

use super::Riscv;

/// Run an ASM instruction and return true if it causes an exception.
macro_rules! noexc_asm {
    (
        $code: literal
        $(, $($params: tt)+)?
    ) => {{
        let exc: usize;
        core::arch::asm!{
            // This will be set to 1 by the exception handler when it detects that the fallible instructions faulted.
            "li a0, 0",
            ".equ __noexc_asm_start, .",
            $code, // Actual instruction to check.
            ".equ __noexc_asm_end, .",
            // This adds it to the table of fallible instructions.
            ".pushsection \".noexc_table\", \"a\", @progbits",
            ".dword __noexc_asm_start",
            ".dword __noexc_asm_end",
            ".popsection"
            // Optional extra in/outs, options, etc.
            $(, $($params)+)?
            // Return value.
            , out("a0") exc
        }
        exc != 0
    }};
}

impl ArchUsermode for Riscv {
    type KernelRegs = ();

    unsafe extern "C" fn enter_usermode(u_pc: usize, u_sp: usize) {
        todo!()
    }

    unsafe extern "C" fn exit_usermode() {
        todo!()
    }

    #[inline(always)]
    fn fallible_load_u8(ptr: *const u8) -> AccessResult<u8> {
        let res;
        if unsafe { noexc_asm!("lbu {}, 0({})", out(reg)res, in(reg)ptr) } {
            return Err(AccessFault);
        }
        Ok(res)
    }

    #[inline(always)]
    fn fallible_load_usize(ptr: *const usize) -> AccessResult<usize> {
        let res;
        if unsafe { noexc_asm!("ld {}, 0({})", out(reg)res, in(reg)ptr) } {
            return Err(AccessFault);
        }
        Ok(res)
    }

    #[inline(always)]
    fn fallible_store_u8(ptr: *const u8, value: u8) -> AccessResult<()> {
        if unsafe { noexc_asm!("sb {}, 0({})", in(reg)value, in(reg)ptr) } {
            return Err(AccessFault);
        }
        Ok(())
    }

    #[inline(always)]
    fn fallible_store_usize(ptr: *const usize, value: usize) -> AccessResult<()> {
        if unsafe { noexc_asm!("sd {}, 0({})", in(reg)value, in(reg)ptr) } {
            return Err(AccessFault);
        }
        Ok(())
    }
}
