# SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
# SPDX-FileType: SOURCE
# SPDX-License-Identifier: MIT



    # Assembly trampoline for calling into usermode from the kernel.
    #
    # Arguments:
    # A0 => pointer to user-mode registers to load (struct GpRegFile)
    # A1 => pointer to kernel-mode context to save (struct ThreadUContext)
    # A2 => interrupt stack out 1/2
    # A3 => interrupt stack out 2/2
    .section ".text.enter_usermode_asm", "ax", @progbits
    .global enter_usermode_asm
    .type enter_usermode_asm, %function
    .align 2
enter_usermode_asm:
    # Saving callee-saved regs to uctx.
    sd ra, 0*8(a1)
    sd sp, 1*8(a1)
    sd s0, 2*8(a1)
    sd s1, 3*8(a1)
    sd s2, 4*8(a1)
    sd s3, 5*8(a1)
    sd s4, 6*8(a1)
    sd s5, 7*8(a1)
    sd s6, 8*8(a1)
    sd s7, 9*8(a1)
    sd s8, 10*8(a1)
    sd s9, 11*8(a1)
    sd s10, 12*8(a1)
    sd s11, 13*8(a1)
    # The old sp is also the new interrupt stack.
    sd sp, 0(a2)
    sd sp, 0(a3)
    
    # Setting up for sret into U-mode.
    li t0, (1<<8)
    csrc sstatus, t0 # -SPP
    li t0, (1<<5)
    csrs sstatus, t0 # +SPIE
    csrw sepc, a0
    
    # Using the scratch space from cpulocal.
    csrr t0, sscratch
    # SP is no longer relevant, so it will temporarily be used
    # to point to the register structs.
    mv sp, a0
    # The section below copied from `exception.S`.
    
    # Load user registers.
    ld t1,   0*8(sp) # PC
    csrw sepc, t1
    ld ra,   1*8(sp)
    ld t1,   2*8(sp) # SP
    sd t1,     8(t0)
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
    
    # Load the last regs.
    ld sp, 8(t0)
    ld t1, 24(t0)
    ld t0, 16(t0)
    
    # Everything read to transition into U-mode.
    sret
    .size enter_usermode_asm, .-enter_usermode_asm
