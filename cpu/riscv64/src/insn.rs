// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#[rustfmt::skip]
pub mod op_maj {
    pub const LOAD:      u32 = 0b00_000;
    pub const LOAD_FP:   u32 = 0b00_001;
    pub const CUSTOM0:   u32 = 0b00_010;
    pub const MISC_MEM:  u32 = 0b00_011;
    pub const OP_IMM:    u32 = 0b00_100;
    pub const AUIPC:     u32 = 0b00_101;
    pub const OP_IMM_32: u32 = 0b00_110;
    pub const STORE:     u32 = 0b01_000;
    pub const STORE_FP:  u32 = 0b01_001;
    pub const CUSTOM1:   u32 = 0b01_010;
    pub const AMO:       u32 = 0b01_011;
    pub const OP:        u32 = 0b01_100;
    pub const LUI:       u32 = 0b01_101;
    pub const OP_32:     u32 = 0b01_110;
    pub const MADD:      u32 = 0b10_000;
    pub const MSUB:      u32 = 0b10_001;
    pub const NMADD:     u32 = 0b10_010;
    pub const NMSUB:     u32 = 0b10_011;
    pub const OP_FP:     u32 = 0b10_100;
    pub const OP_V:      u32 = 0b10_101;
    pub const CUSTOM2:   u32 = 0b10_110;
    pub const BRANCH:    u32 = 0b11_000;
    pub const JALR:      u32 = 0b11_001;
    pub const RESERVED:  u32 = 0b11_010;
    pub const JAL:       u32 = 0b11_011;
    pub const SYSTEM:    u32 = 0b11_100;
    pub const OP_VE:     u32 = 0b11_101;
    pub const CUSTOM3:   u32 = 0b11_110;
}

pub const fn is_float_opc(opc: u16) -> bool {
    match opc & 3 {
        // Taking advantage of a handy overlap in funct3 here.
        0 | 2 => match opc >> 13 {
            // Float load/store.
            0b001 | 0b101 => true,
            // Float load/store on RV32, double-word load/store on RV64.
            #[cfg(target_arch = "riscv32")]
            0b011 | 0b111 => true,
            _ => false,
        },
        _ => false,
    }
}

pub const fn is_float_op(op: u32) -> bool {
    use op_maj::*;
    let op_maj = (op >> 2) & 0x1f;
    let funct3 = (op >> 12) & 0x7;
    let csr = op >> 20;
    match op_maj {
        LOAD_FP | STORE_FP | OP_FP | MADD | MSUB | NMADD | NMSUB => true,
        SYSTEM if funct3 != 0b000 && funct3 != 0b100 => {
            // Is a CSR instruction.
            match csr {
                // Is fflags, frm or fcsr.
                0x001 | 0x002 | 0x003 => true,
                _ => false,
            }
        }
        _ => false,
    }
}
