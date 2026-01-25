

    .text
    .global _start
_start:
#ifdef __riscv
    .option push
    .option norelax
    la gp, __global_pointer$
    .option pop
    
    jal main
    
    # System call: SYSCALL_PROC_EXIT.
    li a7, 8
    ecall
#endif
#ifdef __x86_64__
    .intel_syntax noprefix
    call main

    # System call: SYSCALL_PROC_EXIT.
    mov rdi, rax
    mov rax, 8
    syscall
#endif
    .size _start, .-_start
