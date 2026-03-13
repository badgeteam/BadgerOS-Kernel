
# SPDX-License-Identifier: MIT

#include "cpu/regs.h"
    .global kernel_reg_dump_arr
    .intel_syntax



    # Print a register dump of the current registers.
    .global kernel_cur_regs_dump
    .type kernel_cur_regs_dump, %function
kernel_cur_regs_dump:
    ret



    # Calls the actual trapping code in `isr_noexc_run`.
    .global _isr_noexc_run_int
    .type _isr_noexc_run_int, %function
_isr_noexc_run_int:
    // Save all callee-save regs.
    mov  [%rsi+cpu_regs_t_rbx], %rbx
    mov  [%rsi+cpu_regs_t_rsp], %rsp
    mov  [%rsi+cpu_regs_t_rbp], %rbp
    mov  [%rsi+cpu_regs_t_r12], %r12
    mov  [%rsi+cpu_regs_t_r13], %r13
    mov  [%rsi+cpu_regs_t_r14], %r14
    mov  [%rsi+cpu_regs_t_r15], %r15
    mov  %rax, [%rsp]
    mov  [%rsi+cpu_regs_t_rip], %rax
    // Jump to implementation.
    jmp  %rdx
