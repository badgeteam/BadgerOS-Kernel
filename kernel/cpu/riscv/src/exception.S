# SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
# SPDX-FileType: SOURCE
# SPDX-License-Identifier: MIT



    # Linker relaxation is not possible due to entering from untrusted code,
    # which controls the GP register.
    .option norelax



    # RISC-V interrupt vector table.
    # This must be aligned to a 256-byte boundary, so it is in a special section.
    .section ".interrupt_vector_table", "ax", @progbits
    .global riscv_vector_table
    .type riscv_vector_table, %function
riscv_vector_table:
    .option push
    .option norvc
    .rept 32
    j riscv_exception_asm
    .endr
    .option pop
    .size riscv_vector_table, .-riscv_vector_table



    # Assembly trampoline for entering the kernel from an exception.
    #
    # cpulocal+0  => stack to use (or NULL to use current)
    # cpulocal+8  => temporary storage of SP
    # cpulocal+16 => temporary storage of T0
    # cpulocal+24 => temporary storage of T1
    #
    # T0 => pointer to cpulocal data struct (struct CpuLocal)
    # T1 => Scratch register used to save old value of other regs
    #
    # Entry: Jumped to directly by the CPU as the trap/interrupt vector.
    .section ".text.riscv_exception_asm", "ax", @progbits
    .global riscv_exception_asm
    .type riscv_exception_asm, %function
    .align 2
riscv_exception_asm:
#pragma region
    # Get the cpulocal ptr.
    csrrw t0, sscratch, t0
    # Normal save of T1.
    sd t1, 24(t0)
    # Restore sscratch to be cpulocal ptr again.
    csrrw t1, sscratch, t0
    # T1 used to save old value of T0.
    sd t1, 16(t0)
    # Get the interrupt stack.
    sd sp, 8(t0)
    ld sp, 0(t0)
    # Use old stack ptr if supplied is null (i.e. in kernel ctx).
    bnez sp, 1f
    ld sp, 8(t0)
1:
    # Check for system calls, which use a different codepath
    csrr t1, scause
    addi t1, t1, -8
    beqz t0, riscv_syscall_asm
    
    # Save context to the stack.
    addi sp, sp, -256
    csrr t1, sepc
    sd t1,   0*8(sp) # PC
    sd ra,   1*8(sp)
    ld t1,     8(t0)
    sd t1,   2*8(sp) # SP
    sd gp,   3*8(sp)
    sd tp,   4*8(sp)
    ld t1,    16(t0)
    sd t1,   5*8(sp) # T0
    ld t1,    24(t0)
    sd t1,   6*8(sp) # T1
    sd t2,   7*8(sp)
    sd s0,   8*8(sp)
    sd s1,   9*8(sp)
    sd a0,  10*8(sp)
    sd a1,  11*8(sp)
    sd a2,  12*8(sp)
    sd a3,  13*8(sp)
    sd a4,  14*8(sp)
    sd a5,  15*8(sp)
    sd a6,  16*8(sp)
    sd a7,  17*8(sp)
    sd s2,  18*8(sp)
    sd s3,  19*8(sp)
    sd s4,  20*8(sp)
    sd s5,  21*8(sp)
    sd s6,  22*8(sp)
    sd s7,  23*8(sp)
    sd s8,  24*8(sp)
    sd s9,  25*8(sp)
    sd s10, 26*8(sp)
    sd s11, 27*8(sp)
    sd t3,  28*8(sp)
    sd t4,  29*8(sp)
    sd t5,  30*8(sp)
    sd t6,  31*8(sp)
    addi sp, sp, -32
    csrr t1, sstatus
    sd t1, 0(sp)
    csrr t1, scause
    sd t1, 8(sp)
    csrr t1, stval
    sd t1, 16(sp)
    csrr t1, sepc
    sd t1, 24(sp)
    
    # Call C-ABI exception handler.
    la gp, __global_pointer$
    addi a0, sp, 16 # Pointer to struct GpRegfile
    mv a0, sp # Pointer to struct SpecialRegfile
    jal riscv_exception_handler
    csrr t0, sscratch
    
    # Restore context from the stack.
    ld t1,   0(sp)
    csrw sstatus, t1
    addi sp, sp, 32
    ld t1,   0*8(sp) # PC
    csrw sepc, t1
    ld ra,   1*8(sp)
    ld t1,   2*8(sp) # SP
    sd t1,     0(t0)
    ld gp,   3*8(sp)
    ld tp,   4*8(sp)
    ld t1,   5*8(sp) # T0
    sd t1,    16(t0)
    ld t1,   6*8(sp) # T1
    sd t1,    24(t0)
    ld t2,   7*8(sp)
    ld s0,   8*8(sp)
    ld s1,   9*8(sp)
    ld a0,  10*8(sp)
    ld a1,  11*8(sp)
    ld a2,  12*8(sp)
    ld a3,  13*8(sp)
    ld a4,  14*8(sp)
    ld a5,  15*8(sp)
    ld a6,  16*8(sp)
    ld a7,  17*8(sp)
    ld s2,  18*8(sp)
    ld s3,  19*8(sp)
    ld s4,  20*8(sp)
    ld s5,  21*8(sp)
    ld s6,  22*8(sp)
    ld s7,  23*8(sp)
    ld s8,  24*8(sp)
    ld s9,  25*8(sp)
    ld s10, 26*8(sp)
    ld s11, 27*8(sp)
    ld t3,  28*8(sp)
    ld t4,  29*8(sp)
    ld t5,  30*8(sp)
    ld t6,  31*8(sp)
    addi sp, sp, 256
    
    # Restore the last regs.
    ld sp, 8(t0)
    ld t1, 24(t0)
    ld t0, 16(t0)
    
    # All done, return from exception handler.
    sret
    .size riscv_exception_asm, .-riscv_exception_asm
#pragma endregion



    # Specialization that enters the appropriate system call handler routine.
    #
    # cpulocal+0  => stack to use (or NULL to use current)
    # cpulocal+8  => temporary storage of SP
    # cpulocal+16 => temporary storage of T0
    # cpulocal+24 => temporary storage of T1
    #
    # T0 => pointer to cpulocal data struct (struct CpuLocal)
    # T1 => Scratch register used to save old value of other regs
    #
    # Entry: Kernel SP already loaded, T0 already points to cpulocal.
    # Old SP, T0 and T1 are saved in the cpulocal struct.
    .section ".text.riscv_exception_asm", "ax", @progbits
    .global riscv_syscall_asm
    .type riscv_syscall_asm, %function
    .align 2
riscv_syscall_asm:
#pragma region
    # Save context to the stack.
    addi sp, sp, -16
    ld t1, 8(t0)
    sd t1, 0(sp)
    sd gp, 8(sp)
    
    # Call C-ABI syscall handler.
    la gp, __global_pointer$
    csrsi sstatus, 2 # Set SIE
    jal syscall_handler
    csrci sstatus, 2 # Clear SIE
    csrr t0, sscratch
    
    # Restore context from the stack.
    ld t1, 0(sp)
    sd t1, 8(t0)
    ld gp, 8(sp)
    addi sp, sp, 16
    
    # Replace registers with 0 to avoid leaking kernel state.
    li ra,  0
    li gp,  0
    li tp,  0
    li t2,  0
    li a2,  0
    li a3,  0
    li a4,  0
    li a5,  0
    li a6,  0
    li a7,  0
    li t3,  0
    li t4,  0
    li t5,  0
    li t6,  0
    
    # Restore the last regs.
    ld sp, 8(t0)
    li t1, 0
    li t0, 0
    
    sret
#pragma endregion
