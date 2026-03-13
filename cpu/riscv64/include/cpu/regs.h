
// SPDX-License-Identifier: MIT

#pragma once

#include "cpu/riscv.h"

#ifndef __ASSEMBLER__
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#endif



#if CONFIG_NOMMU

// Exception PC CSR.
#define CSR_EPC     mepc
// Exception PC CSR.
#define CSR_EPC_STR "mepc"

// Exception cause CSR.
#define CSR_CAUSE     mcause
// Exception cause CSR.
#define CSR_CAUSE_STR "mcause"

// Exception value CSR.
#define CSR_TVAL     mtval
// Exception value CSR.
#define CSR_TVAL_STR "mtval"

// Status CSR.
#define CSR_STATUS             mstatus
// Status CSR.
#define CSR_STATUS_STR         "mstatus"
// Interrupt enable bit.
#define CSR_STATUS_IE_BIT      RISCV_STATUS_MIE_BIT
// Interrupt previous bit.
#define CSR_STATUS_PIE_BIT     RISCV_STATUS_MPIE_BIT
// Previous privilege base.
#define CSR_STATUS_PP_BASE_BIT RISCV_STATUS_MPP_BASE_BIT
// Previous privilege mask.
#define CSR_STATUS_PP_MASK     3

// Scratch CSR.
#define CSR_SCRATCH     mscratch
// Scratch CSR.
#define CSR_SCRATCH_STR "mscratch"

// Interrupt pending CSR.
#define CSR_IP     mip
// Interrupt pending CSR.
#define CSR_IP_STR "mip"

// Interrupt enabled CSR.
#define CSR_IE     mie
// Interrupt enabled CSR.
#define CSR_IE_STR "mie"

// Trap/interrupt return instruciton.
#define RISCV_TRAP_RET      mret
// Trap/interrupt return instruciton.
#define RISCV_TRAP_RET_NAME "mret"

#else

// Exception PC CSR.
#define CSR_EPC                sepc
// Exception PC CSR.
#define CSR_EPC_STR            "sepc"

// Exception cause CSR.
#define CSR_CAUSE              scause
// Exception cause CSR.
#define CSR_CAUSE_STR          "scause"

// Exception value CSR.
#define CSR_TVAL               stval
// Exception value CSR.
#define CSR_TVAL_STR           "stval"

// Status CSR.
#define CSR_STATUS             sstatus
// Status CSR.
#define CSR_STATUS_STR         "sstatus"
// Interrupt enable bit.
#define CSR_STATUS_IE_BIT      RISCV_STATUS_SIE_BIT
// Interrupt previous bit.
#define CSR_STATUS_PIE_BIT     RISCV_STATUS_SPIE_BIT
// Previous privilege base.
#define CSR_STATUS_PP_BASE_BIT RISCV_STATUS_SPP_BIT
// Previous privilege mask.
#define CSR_STATUS_PP_MASK     1

// Scratch CSR.
#define CSR_SCRATCH            sscratch
// Scratch CSR.
#define CSR_SCRATCH_STR        "sscratch"

// Interrupt pending CSR.
#define CSR_IP                 sip
// Interrupt pending CSR.
#define CSR_IP_STR             "sip"

// Interrupt enabled CSR.
#define CSR_IE                 sie
// Interrupt enabled CSR.
#define CSR_IE_STR             "sie"

// Address translation CSR.
#define CSR_ATP                satp
// Address translation CSR.
#define CSR_ATP_STR            "satp"

// Trap/interrupt return instruciton.
#define RISCV_TRAP_RET         sret
// Trap/interrupt return instruciton.
#define RISCV_TRAP_RET_NAME    "sret"

#endif



#ifdef __ASSEMBLER__

#define STRUCT_BEGIN(structname)
#if __riscv_xlen == 64
#define STRUCT_FIELD_WORD(structname, name, offset)      .equ structname##_##name, offset * 8
#define STRUCT_FIELD_PTR(structname, type, name, offset) .equ structname##_##name, offset * 8
#else
#define STRUCT_FIELD_WORD(structname, name, offset)      .equ structname##_##name, offset * 4
#define STRUCT_FIELD_PTR(structname, type, name, offset) .equ structname##_##name, offset * 4
#endif
#define STRUCT_END(structname)

#else

#define STRUCT_BEGIN(structname)                         typedef struct structname {
#define STRUCT_FIELD_WORD(structname, name, offset)      size_t name;
#define STRUCT_FIELD_PTR(structname, type, name, offset) type *name;
#define STRUCT_END(structname)                                                                                         \
    }                                                                                                                  \
    structname;

#endif


// RISC-V register file copy.
STRUCT_BEGIN(cpu_regs_t)
STRUCT_FIELD_WORD(cpu_regs_t, pc, 0)
STRUCT_FIELD_WORD(cpu_regs_t, ra, 1)
STRUCT_FIELD_WORD(cpu_regs_t, sp, 2)
STRUCT_FIELD_WORD(cpu_regs_t, gp, 3)
STRUCT_FIELD_WORD(cpu_regs_t, tp, 4)
STRUCT_FIELD_WORD(cpu_regs_t, t0, 5)
STRUCT_FIELD_WORD(cpu_regs_t, t1, 6)
STRUCT_FIELD_WORD(cpu_regs_t, t2, 7)
STRUCT_FIELD_WORD(cpu_regs_t, s0, 8)
STRUCT_FIELD_WORD(cpu_regs_t, s1, 9)
STRUCT_FIELD_WORD(cpu_regs_t, a0, 10)
STRUCT_FIELD_WORD(cpu_regs_t, a1, 11)
STRUCT_FIELD_WORD(cpu_regs_t, a2, 12)
STRUCT_FIELD_WORD(cpu_regs_t, a3, 13)
STRUCT_FIELD_WORD(cpu_regs_t, a4, 14)
STRUCT_FIELD_WORD(cpu_regs_t, a5, 15)
STRUCT_FIELD_WORD(cpu_regs_t, a6, 16)
STRUCT_FIELD_WORD(cpu_regs_t, a7, 17)
STRUCT_FIELD_WORD(cpu_regs_t, s2, 18)
STRUCT_FIELD_WORD(cpu_regs_t, s3, 19)
STRUCT_FIELD_WORD(cpu_regs_t, s4, 20)
STRUCT_FIELD_WORD(cpu_regs_t, s5, 21)
STRUCT_FIELD_WORD(cpu_regs_t, s6, 22)
STRUCT_FIELD_WORD(cpu_regs_t, s7, 23)
STRUCT_FIELD_WORD(cpu_regs_t, s8, 24)
STRUCT_FIELD_WORD(cpu_regs_t, s9, 25)
STRUCT_FIELD_WORD(cpu_regs_t, s10, 26)
STRUCT_FIELD_WORD(cpu_regs_t, s11, 27)
STRUCT_FIELD_WORD(cpu_regs_t, t3, 28)
STRUCT_FIELD_WORD(cpu_regs_t, t4, 29)
STRUCT_FIELD_WORD(cpu_regs_t, t5, 30)
STRUCT_FIELD_WORD(cpu_regs_t, t6, 31)
STRUCT_END(cpu_regs_t)


#ifndef __ASSEMBLER__
typedef cpu_regs_t cpu_regs_t;
#endif

#define REGNO_PC  0
#define REGNO_RA  1
#define REGNO_SP  2
#define REGNO_GP  3
#define REGNO_TP  4
#define REGNO_T0  5
#define REGNO_T1  6
#define REGNO_T2  7
#define REGNO_S0  8
#define REGNO_S1  9
#define REGNO_A0  10
#define REGNO_A1  11
#define REGNO_A2  12
#define REGNO_A3  13
#define REGNO_A4  14
#define REGNO_A5  15
#define REGNO_A6  16
#define REGNO_A7  17
#define REGNO_S2  18
#define REGNO_S3  19
#define REGNO_S4  20
#define REGNO_S5  21
#define REGNO_S6  22
#define REGNO_S7  23
#define REGNO_S8  24
#define REGNO_S9  25
#define REGNO_S10 26
#define REGNO_S11 27
#define REGNO_T3  28
#define REGNO_T4  29
#define REGNO_T5  30
#define REGNO_T6  31

#undef STRUCT_BEGIN
#undef STRUCT_FIELD_WORD
#undef STRUCT_FIELD_PTR
#undef STRUCT_END
