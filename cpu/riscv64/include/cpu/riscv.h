
// SPDX-License-Identifier: MIT

#pragma once

// Undef x86 macros because intellisense doesn't recognise RISC-V.
#ifndef __riscv_xlen
#ifdef __x86_64__
#define __riscv_xlen 64
#else
#define __riscv_xlen 32
#endif
#endif
#undef __x86_64__
#undef __i386__
#ifndef __riscv
#define __riscv
#endif



/* ==== RISC-V MSTATUS DEFINITION ==== */

#define RISCV_STATUS_SIE_BIT      1
#define RISCV_STATUS_MIE_BIT      3
#define RISCV_STATUS_SPIE_BIT     5
#define RISCV_STATUS_UBE_BIT      6
#define RISCV_STATUS_MPIE_BIT     7
#define RISCV_STATUS_SPP_BIT      8
#define RISCV_STATUS_VS_BASE_BIT  9  // ,10
#define RISCV_STATUS_MPP_BASE_BIT 11 // ,12
#define RISCV_STATUS_FS_BASE_BIT  13 // ,14
#define RISCV_STATUS_XS_BASE_BIT  15 // ,16
#define RISCV_STATUS_MPRV_BIT     17
#define RISCV_STATUS_SUM_BIT      18
#define RISCV_STATUS_MXR_BIT      19
#define RISCV_STATUS_TVM_BIT      20
#define RISCV_STATUS_TW_BIT       21
#define RISCV_STATUS_TSR_BIT      22
#define RISCV_STATUS_SR_BIT       31



/* ==== RISC-V INTERRUPT LIST ==== */
#define RISCV_INT_SUPERVISOR_SOFT  1
#define RISCV_INT_MACHINE_SOFT     3
#define RISCV_INT_SUPERVISOR_TIMER 5
#define RISCV_INT_MACHINE_TIMER    7
#define RISCV_INT_SUPERVISOR_EXT   9
#define RISCV_INT_MACHINE_EXT      11

#if RISCV_M_MODE_KERNEL
#define RISCV_INT_SOFT       RISCV_INT_MACHINE_SOFT
#define RISCV_INT_EXT        RISCV_INT_MACHINE_EXT
#define RISCV_INT_TIMER      RISCV_INT_MACHINE_TIMER
#define RISCV_STATUS_IE_BIT  RISCV_STATUS_MIE_BIT
#define RISCV_STATUS_PIE_BIT RISCV_STATUS_MPIE_BIT
#else
#define RISCV_INT_SOFT       RISCV_INT_SUPERVISOR_SOFT
#define RISCV_INT_EXT        RISCV_INT_SUPERVISOR_EXT
#define RISCV_INT_TIMER      RISCV_INT_SUPERVISOR_TIMER
#define RISCV_STATUS_IE_BIT  RISCV_STATUS_SIE_BIT
#define RISCV_STATUS_PIE_BIT RISCV_STATUS_SPIE_BIT
#endif



/* ==== RISC-V TRAP LIST ==== */

// Instruction access misaligned.
#define RISCV_TRAP_IALIGN   0x00
// Instruction access fault.
#define RISCV_TRAP_IACCESS  0x01
// Illegal instruction.
#define RISCV_TRAP_IILLEGAL 0x02
// Trace / breakpoint trap.
#define RISCV_TRAP_EBREAK   0x03
// Load access misaligned.
#define RISCV_TRAP_LALIGN   0x04
// Load access fault.
#define RISCV_TRAP_LACCESS  0x05
// Store / AMO access misaligned.
#define RISCV_TRAP_SALIGN   0x06
// Store / AMO access fault.
#define RISCV_TRAP_SACCESS  0x07
// ECALL from U-mode.
#define RISCV_TRAP_U_ECALL  0x08
// ECALL from S-mode.
#define RISCV_TRAP_S_ECALL  0x09
// ECALL from M-mode.
#define RISCV_TRAP_M_ECALL  0x0B
// Instruction page fault.
#define RISCV_TRAP_IPAGE    0x0C
// Load page fault.
#define RISCV_TRAP_LPAGE    0x0D
// Store / AMO page fault.
#define RISCV_TRAP_SPAGE    0x0F
