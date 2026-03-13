
// SPDX-License-Identifier: MIT

#pragma once

#ifndef __ASSEMBLER__
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#endif



// Access control (rings 0-2) / alignment check (ring 3) flag.
#define RFLAGS_AC        (1 << 18)
// Interrupt enable flag.
#define RFLAGS_IF        (1 << 9)
// I/O permission level base bit.
#define RFLAGS_IOPL_BASE 12
// I/O permission level mask.
#define RFLAGS_IOPL_MASK (3 << 12)



#ifdef __ASSEMBLER__

#define STRUCT_BEGIN(structname) .equ structname##_size, 0
#define STRUCT_FIELD(structname, name, len)                                                                            \
    .equ structname##_##name, structname##_size;                                                                       \
    .equ structname##_size, structname##_size + len
#define STRUCT_FIELD_QWORD(structname, name)     STRUCT_FIELD(structname, name, 8)
#define STRUCT_FIELD_DWORD(structname, name)     STRUCT_FIELD(structname, name, 4)
#define STRUCT_FIELD_WORD(structname, name)      STRUCT_FIELD(structname, name, 2)
#define STRUCT_FIELD_PTR(structname, type, name) STRUCT_FIELD(structname, name, 8)
#define STRUCT_END(structname)

#else

#define STRUCT_BEGIN(structname)                 typedef struct {
#define STRUCT_FIELD_QWORD(structname, name)     uint64_t name;
#define STRUCT_FIELD_DWORD(structname, name)     uint32_t name;
#define STRUCT_FIELD_WORD(structname, name)      uint16_t name;
#define STRUCT_FIELD_PTR(structname, type, name) type *name;
#define STRUCT_END(structname)                                                                                         \
    }                                                                                                                  \
    structname;

#endif

// AMD64 register file copy.
STRUCT_BEGIN(cpu_regs_t)
STRUCT_FIELD_QWORD(cpu_regs_t, rax)
STRUCT_FIELD_QWORD(cpu_regs_t, rbx)
STRUCT_FIELD_QWORD(cpu_regs_t, rcx)
STRUCT_FIELD_QWORD(cpu_regs_t, rdx)
STRUCT_FIELD_QWORD(cpu_regs_t, rsi)
STRUCT_FIELD_QWORD(cpu_regs_t, rdi)
STRUCT_FIELD_QWORD(cpu_regs_t, rsp)
STRUCT_FIELD_QWORD(cpu_regs_t, rbp)
STRUCT_FIELD_QWORD(cpu_regs_t, r8)
STRUCT_FIELD_QWORD(cpu_regs_t, r9)
STRUCT_FIELD_QWORD(cpu_regs_t, r10)
STRUCT_FIELD_QWORD(cpu_regs_t, r11)
STRUCT_FIELD_QWORD(cpu_regs_t, r12)
STRUCT_FIELD_QWORD(cpu_regs_t, r13)
STRUCT_FIELD_QWORD(cpu_regs_t, r14)
STRUCT_FIELD_QWORD(cpu_regs_t, r15)
STRUCT_FIELD_QWORD(cpu_regs_t, rip)
STRUCT_FIELD_QWORD(cpu_regs_t, fsbase)
STRUCT_FIELD_QWORD(cpu_regs_t, gsbase)
STRUCT_FIELD_QWORD(cpu_regs_t, rflags)
STRUCT_FIELD_WORD(cpu_regs_t, cs)
STRUCT_FIELD_WORD(cpu_regs_t, ss)
STRUCT_FIELD_WORD(cpu_regs_t, ds)
STRUCT_FIELD_WORD(cpu_regs_t, es)
STRUCT_FIELD_WORD(cpu_regs_t, fs)
STRUCT_FIELD_WORD(cpu_regs_t, gs)
STRUCT_END(cpu_regs_t)



// AMD64 automatically-pushed interrupt context.
// This is used primarily by assembly.
STRUCT_BEGIN(amd64_irqframe_t)
STRUCT_FIELD_QWORD(amd64_irqframe_t, rip)
STRUCT_FIELD_QWORD(amd64_irqframe_t, cs)
STRUCT_FIELD_QWORD(amd64_irqframe_t, rflags)
STRUCT_FIELD_QWORD(amd64_irqframe_t, rsp)
STRUCT_FIELD_QWORD(amd64_irqframe_t, ss)
STRUCT_END(amd64_irqframe_t)



#undef STRUCT_BEGIN
#undef STRUCT_FIELD
#undef STRUCT_FIELD_WORD
#undef STRUCT_FIELD_DWORD
#undef STRUCT_FIELD_QWORD
#undef STRUCT_FIELD_PTR
#undef STRUCT_END
