
# SPDX-License-Identifier: MIT

#include "cpu/isr_ctx.h"
    .intel_syntax



    .text
    .align 2
    .global amd64_syscall_entry
    .type amd64_syscall_entry, %function
    # The entrypoint of AMD64 system calls; sets up some registers and transfers control to C.
    # Syscall has saved RFLAGS to R11 and saves the next instruction's address to RCX.
    # Because RCX is not available for the SYSV ABI, R10 is used instead.
amd64_syscall_entry:
    # Use swapgs to get access to the ISR context.
    swapgs
    
    # Save RIP, RSP and RFLAGS.
    mov  [%gs:isr_ctx_t_regs+cpu_regs_t_rip], %rcx
    mov  [%gs:isr_ctx_t_regs+cpu_regs_t_rsp], %rsp
    mov  [%gs:isr_ctx_t_regs+cpu_regs_t_rbp], %rbp
    mov  [%gs:isr_ctx_t_regs+cpu_regs_t_rflags], %r11
    
    # Grab new stack.
    mov  %rsp, [%gs:isr_ctx_t_syscall_stack]
    mov  %rbp, 0
    
    # Having to save these registers is an inefficiency that would require a re-work of signals to fix.
    mov  [%gs:isr_ctx_t_regs+cpu_regs_t_rbx], %rbx
    mov  [%gs:isr_ctx_t_regs+cpu_regs_t_r12], %r12
    mov  [%gs:isr_ctx_t_regs+cpu_regs_t_r13], %r13
    mov  [%gs:isr_ctx_t_regs+cpu_regs_t_r14], %r14
    mov  [%gs:isr_ctx_t_regs+cpu_regs_t_r15], %r15
    
    push %rdi
    push %rsi
    push %rdx
    push %r10 # Move R10 back into RCX so we can directly call the syscall impl.
    push %r8
    push %r9
    
    # Get system call information.
    # RDX doesn't need to be saved here and will be zeroed later.
    mov  %rdi, %rax
    call syscall_info
    
    # Swap context to the kernel one.
    push %rax
    push %rdx
    call amd64_syscall_raise
    pop  %rdx
    pop  %rax
    sti
    
    pop  %r9
    pop  %r8
    pop  %rcx
    pop  %rdx
    pop  %rsi
    pop  %rdi
    
    # Run system call handler.
    push %rdx # Save args size from syscall info.
    call %rax
    pop  %rdi # Move it into rdi because it's free now.
    
    # Implicit return from syscall will return 0.
    and  %rdi, 0xff
    cmp  %rdi, 0
    jne  1f
    
    # Returns void; clear both RAX and RDX.
    xor  %rdx, %rdx
    xor  %rax, %rax
    jmp  2f
1:
    cmp  %rdi, 8
    jge  2f
    
    # Returns <= 8 bytes; clear only RDX.
    xor  %rdx, %rdx
2:
    # Returns <= 16 bytes; don't clear RAX nor RDX.
    
    # Swap context back to the user one.
    cli
    push %rax
    push %rdx
    call amd64_syscall_lower
    pop  %rdx
    pop  %rax
    
    # Clear temporary registers.
    xor  %rdi, %rdi
    xor  %rsi, %rsi
    xor  %r8, %r8
    xor  %r9, %r9
    xor  %r10, %r10
    
    # Restore callee-save registers.
    mov  %rcx, [%gs:isr_ctx_t_regs+cpu_regs_t_rip]
    mov  %rsp, [%gs:isr_ctx_t_regs+cpu_regs_t_rsp]
    mov  %rbp, [%gs:isr_ctx_t_regs+cpu_regs_t_rbp]
    mov  %r11, [%gs:isr_ctx_t_regs+cpu_regs_t_rflags]
    
    # Return from system call.
    swapgs
    sysretq
    .size amd64_syscall_entry, .-amd64_syscall_entry