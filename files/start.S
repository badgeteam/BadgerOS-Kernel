

    .text
    .global _start
_start:
    .option push
    .option norelax
    la gp, __global_pointer$
    .option pop
    
    jal main
    
    # System call: SYSCALL_PROC_EXIT.
    li a7, 8
    ecall
    .size _start, .-_start
