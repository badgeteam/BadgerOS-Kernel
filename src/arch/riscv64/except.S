# SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
# SPDX-FileType: SOURCE
# SPDX-License-Identifier: MIT



    .cfi_sections .debug_frame

    # Linker relaxation is not possible due to entering from untrusted code,
    # which controls the GP register.
    .option norelax



    # RISC-V interrupt vector table.
    # This must be aligned to a 128-byte boundary, so it is in a special section.
    .section ".interrupt_vector_table", "ax", @progbits
    .global riscv_exception_vector
    .type riscv_exception_vector, %function
    .align 7
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
    # Entry: Jumped to directly by the CPU as the trap/interrupt vector.
    .section ".interrupt_vector_table", "ax", @progbits
    .global riscv_exception_asm
    .type riscv_exception_asm, %function
    .align 2
riscv_exception_asm:
    .cfi_startproc
    .cfi_return_column 64
    .cfi_signal_frame
    .cfi_register 64, 0x1141 # sepc
    # Get the cpulocal ptr.
    csrrw   tp, sscratch, tp                    # Swap cpulocal ptr and tp.
    sd      t0, {ArchCpuLocalData_old_t0}(tp)   # Save old t0.
    csrrw   t0, sscratch, tp                    # Get old tp and restore cpulocal ptr.
    sd      t0, {ArchCpuLocalData_old_tp}(tp)   # Save old tp.
    sd      sp, {ArchCpuLocalData_old_sp}(tp)   # Save old sp.
    
    # If from U-mode, get the stack.
    csrr    t0, sstatus
    andi    t0, t0, {sstatus_spp_mask}          # S-mode Previous Privilege.
    bnez    t0, 1f                              # If 1, S-mode; do not load sp.
    ld      sp, {ArchCpuLocalData_irq_sp}(tp)   # If 0, U-mode; load sp.
1:
    # Save all registers.
    addi    sp,  sp, -{RiscvExceptFrame_size}
    .cfi_def_cfa sp, 0
    csrr    t0,  sepc
    sd      t0,  {RiscvExceptFrame_pc}(sp)      # Old PC.
    sd      ra,  {RiscvExceptFrame_ra}(sp)
    ld      t0,  {ArchCpuLocalData_old_sp}(tp)
    sd      t0,  {RiscvExceptFrame_sp}(sp)      # Old sp.
    sd      gp,  {RiscvExceptFrame_gp}(sp)
    ld      t0,  {ArchCpuLocalData_old_tp}(tp)
    sd      t0,  {RiscvExceptFrame_tp}(sp)      # Old tp.
    ld      t0,  {ArchCpuLocalData_old_t0}(tp)
    sd      t0,  {RiscvExceptFrame_t0}(sp)      # Old t0.
    sd      t1,  {RiscvExceptFrame_t1}(sp)
    sd      t2,  {RiscvExceptFrame_t2}(sp)
    sd      s0,  {RiscvExceptFrame_s0}(sp)
    sd      s1,  {RiscvExceptFrame_s1}(sp)
    sd      a0,  {RiscvExceptFrame_a0}(sp)
    sd      a1,  {RiscvExceptFrame_a1}(sp)
    sd      a2,  {RiscvExceptFrame_a2}(sp)
    sd      a3,  {RiscvExceptFrame_a3}(sp)
    sd      a4,  {RiscvExceptFrame_a4}(sp)
    sd      a5,  {RiscvExceptFrame_a5}(sp)
    sd      a6,  {RiscvExceptFrame_a6}(sp)
    sd      a7,  {RiscvExceptFrame_a7}(sp)
    sd      s2,  {RiscvExceptFrame_s2}(sp)
    sd      s3,  {RiscvExceptFrame_s3}(sp)
    sd      s4,  {RiscvExceptFrame_s4}(sp)
    sd      s5,  {RiscvExceptFrame_s5}(sp)
    sd      s6,  {RiscvExceptFrame_s6}(sp)
    sd      s7,  {RiscvExceptFrame_s7}(sp)
    sd      s8,  {RiscvExceptFrame_s8}(sp)
    sd      s9,  {RiscvExceptFrame_s9}(sp)
    sd      s10, {RiscvExceptFrame_s10}(sp)
    sd      s11, {RiscvExceptFrame_s11}(sp)
    sd      t3,  {RiscvExceptFrame_t3}(sp)
    sd      t4,  {RiscvExceptFrame_t4}(sp)
    sd      t5,  {RiscvExceptFrame_t5}(sp)
    sd      t6,  {RiscvExceptFrame_t6}(sp)
    .cfi_rel_offset 64,  {RiscvExceptFrame_pc}
    .cfi_rel_offset ra,  {RiscvExceptFrame_ra}
    .cfi_rel_offset sp,  {RiscvExceptFrame_sp}
    .cfi_rel_offset gp,  {RiscvExceptFrame_gp}
    .cfi_rel_offset tp,  {RiscvExceptFrame_tp}
    .cfi_rel_offset t0,  {RiscvExceptFrame_t0}
    .cfi_rel_offset t1,  {RiscvExceptFrame_t1}
    .cfi_rel_offset t2,  {RiscvExceptFrame_t2}
    .cfi_rel_offset s0,  {RiscvExceptFrame_s0}
    .cfi_rel_offset s1,  {RiscvExceptFrame_s1}
    .cfi_rel_offset a0,  {RiscvExceptFrame_a0}
    .cfi_rel_offset a1,  {RiscvExceptFrame_a1}
    .cfi_rel_offset a2,  {RiscvExceptFrame_a2}
    .cfi_rel_offset a3,  {RiscvExceptFrame_a3}
    .cfi_rel_offset a4,  {RiscvExceptFrame_a4}
    .cfi_rel_offset a5,  {RiscvExceptFrame_a5}
    .cfi_rel_offset a6,  {RiscvExceptFrame_a6}
    .cfi_rel_offset a7,  {RiscvExceptFrame_a7}
    .cfi_rel_offset s2,  {RiscvExceptFrame_s2}
    .cfi_rel_offset s3,  {RiscvExceptFrame_s3}
    .cfi_rel_offset s4,  {RiscvExceptFrame_s4}
    .cfi_rel_offset s5,  {RiscvExceptFrame_s5}
    .cfi_rel_offset s6,  {RiscvExceptFrame_s6}
    .cfi_rel_offset s7,  {RiscvExceptFrame_s7}
    .cfi_rel_offset s8,  {RiscvExceptFrame_s8}
    .cfi_rel_offset s9,  {RiscvExceptFrame_s9}
    .cfi_rel_offset s10, {RiscvExceptFrame_s10}
    .cfi_rel_offset s11, {RiscvExceptFrame_s11}
    .cfi_rel_offset t3,  {RiscvExceptFrame_t3}
    .cfi_rel_offset t4,  {RiscvExceptFrame_t4}
    .cfi_rel_offset t5,  {RiscvExceptFrame_t5}
    .cfi_rel_offset t6,  {RiscvExceptFrame_t6}
    csrr    t0,  sstatus
    sd      t0,  {RiscvExceptFrame_sstatus}(sp)
    csrr    t0,  scause
    sd      t0,  {RiscvExceptFrame_scause}(sp)
    csrr    t0,  stval
    sd      t0,  {RiscvExceptFrame_stval}(sp)
    mv      a0,  sp                             # Copy pointer to RiscvExceptFrame.
    
    # Fake stack frame for unwinding.
    addi    sp, sp, -16
    .cfi_adjust_cfa_offset 16
    csrr    t0, sepc
    addi    t0, t0, 1                           # Mark as interrupted for the unwinder.
    sd      t0, 0(sp)
    sd      s0, 8(sp)
    addi    s0, sp, 16
    sd      s0, {RiscvExceptFrame_fake_fp}(a0)
    
    # Call C-abi exception handler.
    la      gp, __global_pointer$
    li      t0, {sstatus_clear}                 # Disable float, vector, etc. before entering kernel.
    csrc    sstatus, t0
    call    riscv_exception_handler
    
    addi    sp, sp, 16
    
    # Restore all registers.
    ld      t0,  {RiscvExceptFrame_sstatus}(sp)
    csrw    sstatus, t0
    ld      t0,  {RiscvExceptFrame_scause}(sp)
    csrw    scause,  t0
    ld      t0,  {RiscvExceptFrame_stval}(sp)
    csrw    stval,   t0
    ld      t0,  {RiscvExceptFrame_pc}(sp)
    csrw    sepc,    t0
    ld      ra,  {RiscvExceptFrame_ra}(sp)
    ld      gp,  {RiscvExceptFrame_gp}(sp)
    ld      tp,  {RiscvExceptFrame_tp}(sp)
    ld      t0,  {RiscvExceptFrame_t0}(sp)
    ld      t1,  {RiscvExceptFrame_t1}(sp)
    ld      t2,  {RiscvExceptFrame_t2}(sp)
    ld      s0,  {RiscvExceptFrame_s0}(sp)
    ld      s1,  {RiscvExceptFrame_s1}(sp)
    ld      a0,  {RiscvExceptFrame_a0}(sp)
    ld      a1,  {RiscvExceptFrame_a1}(sp)
    ld      a2,  {RiscvExceptFrame_a2}(sp)
    ld      a3,  {RiscvExceptFrame_a3}(sp)
    ld      a4,  {RiscvExceptFrame_a4}(sp)
    ld      a5,  {RiscvExceptFrame_a5}(sp)
    ld      a6,  {RiscvExceptFrame_a6}(sp)
    ld      a7,  {RiscvExceptFrame_a7}(sp)
    ld      s2,  {RiscvExceptFrame_s2}(sp)
    ld      s3,  {RiscvExceptFrame_s3}(sp)
    ld      s4,  {RiscvExceptFrame_s4}(sp)
    ld      s5,  {RiscvExceptFrame_s5}(sp)
    ld      s6,  {RiscvExceptFrame_s6}(sp)
    ld      s7,  {RiscvExceptFrame_s7}(sp)
    ld      s8,  {RiscvExceptFrame_s8}(sp)
    ld      s9,  {RiscvExceptFrame_s9}(sp)
    ld      s10, {RiscvExceptFrame_s10}(sp)
    ld      s11, {RiscvExceptFrame_s11}(sp)
    ld      t3,  {RiscvExceptFrame_t3}(sp)
    ld      t4,  {RiscvExceptFrame_t4}(sp)
    ld      t5,  {RiscvExceptFrame_t5}(sp)
    ld      t6,  {RiscvExceptFrame_t6}(sp)
    
    ld      sp,  {RiscvExceptFrame_sp}(sp)
    sret
    .cfi_endproc
    .size riscv_exception_asm, .-riscv_exception_asm
