# SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
# SPDX-FileType: SOURCE
# SPDX-License-Identifier: MIT



    .cfi_sections .debug_frame

    # Linker relaxation is not possible due to entering from untrusted code,
    # which controls the GP register.
    .option norelax



    # RISC-V interrupt vector table.
    # This must be aligned to a 256-byte boundary, so it is in a special section.
    .section ".interrupt_vector_table", "ax", @progbits
    .global riscv_exception_vector
    .type riscv_exception_vector, %function
riscv_exception_vector:
    .option push
    .option norvc
    .rept 32
    j riscv_exception_asm
    .endr
    .option pop
    .size riscv_exception_vector, .-riscv_exception_vector



    # Assembly trampoline for entering the kernel from an exception.
    #
    # cpulocal+0  => temporary storage of t0
    # cpulocal+8  => temporary storage of sp
    # cpulocal+16 => temporary storage of tp
    # cpulocal+24 => stack to use if emerging from U-mode
    #
    # Entry: Jumped to directly by the CPU as the trap/interrupt vector.
    .section ".text.riscv_exception_asm", "ax", @progbits
    .global riscv_exception_asm
    .type riscv_exception_asm, %function
    .align 2
    .cfi_startproc
riscv_exception_asm:
    .cfi_return_column 64
    .cfi_register 64, sepc
    # Initial entry: Get a foothold in the kernel.
    csrrw tp, sscratch, tp  # Swap user tp and cpu-local ptr.
    sd    t0, 0(tp)         # We will use t0 as a temporary.
    sd    sp, 8(tp)         # We be setting sp to the kernel stack ptr.
    csrrw t0, sscratch, tp  # Save user tp.
    sd    t0, 16(tp)
    csrr  t0, sstatus       # Test SSTATUS.SPP.
    andi  t0, t0, 1 << 8
    bnez  t0, 1f            # If 1, from S-mode; keep sp.
    ld    sp, 24(tp)        # If 0, from U-mode; load sp from cpulocal.
1:
    .cfi_remember_state

    # Save context to the stack.
    addi sp, sp, -288
    .cfi_def_cfa sp, 0
    csrr t1, sepc
    sd t1,   0*8(sp) # PC
    .cfi_rel_offset 64, 0*8
    sd ra,   1*8(sp)
    .cfi_rel_offset ra, 1*8
    ld t1,     8(tp)
    sd t1,   2*8(sp)        # Old sp
    .cfi_rel_offset sp, 2*8
    sd gp,   3*8(sp)
    .cfi_rel_offset gp, 3*8
    ld t0,    16(tp)
    sd t0,   4*8(sp)        # Old tp
    .cfi_rel_offset tp, 4*8
    ld t0,     0(tp)
    sd t0,   5*8(sp)        # Old t0
    .cfi_rel_offset t0, 5*8
    sd t1,   6*8(sp)
    .cfi_rel_offset t1, 6*8
    sd t2,   7*8(sp)
    .cfi_rel_offset t2, 7*8
    sd s0,   8*8(sp)
    .cfi_rel_offset s0, 8*8
    sd s1,   9*8(sp)
    .cfi_rel_offset s1, 9*8
    sd a0,  10*8(sp)
    .cfi_rel_offset a0, 10*8
    sd a1,  11*8(sp)
    .cfi_rel_offset a1, 11*8
    sd a2,  12*8(sp)
    .cfi_rel_offset a2, 12*8
    sd a3,  13*8(sp)
    .cfi_rel_offset a3, 13*8
    sd a4,  14*8(sp)
    .cfi_rel_offset a4, 14*8
    sd a5,  15*8(sp)
    .cfi_rel_offset a5, 15*8
    sd a6,  16*8(sp)
    .cfi_rel_offset a6, 16*8
    sd a7,  17*8(sp)
    .cfi_rel_offset a7, 17*8
    sd s2,  18*8(sp)
    .cfi_rel_offset s2, 18*8
    sd s3,  19*8(sp)
    .cfi_rel_offset s3, 19*8
    sd s4,  20*8(sp)
    .cfi_rel_offset s4, 20*8
    sd s5,  21*8(sp)
    .cfi_rel_offset s5, 21*8
    sd s6,  22*8(sp)
    .cfi_rel_offset s6, 22*8
    sd s7,  23*8(sp)
    .cfi_rel_offset s7, 23*8
    sd s8,  24*8(sp)
    .cfi_rel_offset s8, 24*8
    sd s9,  25*8(sp)
    .cfi_rel_offset s9, 25*8
    sd s10, 26*8(sp)
    .cfi_rel_offset s10, 26*8
    sd s11, 27*8(sp)
    .cfi_rel_offset s11, 27*8
    sd t3,  28*8(sp)
    .cfi_rel_offset t3, 28*8
    sd t4,  29*8(sp)
    .cfi_rel_offset t4, 29*8
    sd t5,  30*8(sp)
    .cfi_rel_offset t5, 30*8
    sd t6,  31*8(sp)
    .cfi_rel_offset t6, 31*8
    csrr t1, sstatus
    sd   t1, 33*8(sp)
    csrr t1, scause
    sd   t1, 34*8(sp)
    csrr t1, stval
    sd   t1, 35*8(sp)
    
    # Create a fake stack frame for kernel unwinding.
    addi sp, sp, -16
    .cfi_adjust_cfa_offset 16
    csrr t1, sepc
    addi t1, t1, 1
    sd   t1, 8(sp) # pc
    sd   s0, 0(sp) # fp
    addi s0, sp, 16
    sd   s0, 16+32*8(sp)    # Fake frame ptr.
    
    # Disable extensions before re-entry into the kernel.
    #           VS         FS          XS
    li   t1, (3 << 9) | (3 << 13) | (3 << 15)
    csrc sstatus, t1
    
    # Call Rust exception handler.
    la gp, __global_pointer$
    addi a0, sp, 16         # Pointer to struct RiscvExceptionFrame
    la   ra, riscv_exception_handler
    jalr ra
    
    # Delete fake unwind frame.
    addi sp, sp, 16
    
    # Restore context from the stack.
    ld t1,  33*8(sp)
    csrw sstatus, t1
    ld t1,   0*8(sp) # pc
    csrw sepc, t1
    ld ra,   1*8(sp)
    #  sp    2*8(sp) # deferred
    ld gp,   3*8(sp)
    #  tp    4*8(sp) # deferred
    #  t0    5*8(sp) # deferred
    ld t1,   6*8(sp)
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
    
    # Restore the last regs.
    ld tp,   4*8(sp)
    ld t0,   5*8(sp)
    ld sp,   2*8(sp)
    .cfi_restore_state
    
    # All done, return from exception handler.
    sret
    .cfi_endproc
    .size riscv_exception_asm, .-riscv_exception_asm
