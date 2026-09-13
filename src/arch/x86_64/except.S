# SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
# SPDX-FileType: SOURCE
# SPDX-License-Identifier: MIT



    .cfi_sections .debug_frame
    .global x86_irq_handler



    # IDT stub addresses.
    .section ".rodata", "a"
    .global x86_idtstubs
    .p2align 2
x86_idtstubs:
    .rept 256
    .8byte x86_idtstub_\+
    .endr
    .size x86_idtstubs, .-x86_idtstubs



    # Macro for IRQs with error codes.
    .macro irq_ec index
    .section ".interrupt_vector_table", "ax", @progbits
    .type x86_idtstub_\index, %function
x86_idtstub_\index:
    push \index
    jmp  x86_irq_asm
    .size x86_idtstub_\index, .-x86_idtstub_\index
    .endm



    # Macro for IRQs without error codes.
    .macro irq_noec index
    .section ".interrupt_vector_table", "ax", @progbits
    .type x86_idtstub_\index, %function
x86_idtstub_\index:
    push 0
    push \index
    jmp  x86_irq_asm
    .size x86_idtstub_\index, .-x86_idtstub_\index
    .endm



    # The pre-defined error codes.
    irq_noec  0     # DE
    irq_noec  1     # DB
    irq_noec  2     # NMI
    irq_noec  3     # BP
    irq_noec  4     # OF
    irq_noec  5     # BR
    irq_noec  6     # UD
    irq_noec  7     # NM
    irq_ec    8     # DF
    irq_noec  9     # Coprocessor segment overrun
    irq_ec   10     # TS
    irq_ec   11     # NP
    irq_ec   12     # SS
    irq_ec   13     # GP
    irq_ec   14     # PF
    irq_noec 15     # Reserved
    irq_noec 16     # MF
    irq_ec   17     # AC
    irq_noec 18     # MC
    irq_noec 19     # XF
    irq_noec 20     # Reserved
    irq_ec   21     # CP
    irq_noec 22     # Reserved
    irq_noec 23     # Reserved
    irq_noec 24     # Reserved
    irq_noec 25     # Reserved
    irq_noec 26     # Reserved
    irq_noec 27     # Reserved
    irq_noec 28     # HV
    irq_noec 29     # VC
    irq_noec 30     # SX
    irq_noec 31     # Reserved
    .rept 256       # User-defined
    .if \+ >= 32
    irq_noec \+
    .endif
    .endr



    # Generic IDT stub.
    .section ".interrupt_vector_table", "ax", @progbits
    .type x86_irq_asm, %function
    .p2align 2
x86_irq_asm:
    .cfi_startproc
    .cfi_def_cfa rsp, -16*8
    .cfi_signal_frame
    .cfi_return_column rip
    .cfi_offset rip, {X86TrapFrame_rip}
    .cfi_offset rsp, {X86TrapFrame_rsp}
    
    # Save GPRs.
    push r15
    .cfi_adjust_cfa_offset 8
    push r14
    .cfi_adjust_cfa_offset 8
    push r13
    .cfi_adjust_cfa_offset 8
    push r12
    .cfi_adjust_cfa_offset 8
    push r11
    .cfi_adjust_cfa_offset 8
    push r10
    .cfi_adjust_cfa_offset 8
    push r9
    .cfi_adjust_cfa_offset 8
    push r8
    .cfi_adjust_cfa_offset 8
    push rdi
    .cfi_adjust_cfa_offset 8
    push rsi
    .cfi_adjust_cfa_offset 8
    push rbp
    .cfi_adjust_cfa_offset 8
    push rdx
    .cfi_adjust_cfa_offset 8
    push rcx
    .cfi_adjust_cfa_offset 8
    push rbx
    .cfi_adjust_cfa_offset 8
    push rax
    .cfi_adjust_cfa_offset 8
    .cfi_remember_state
    .cfi_offset r15, {X86TrapFrame_r15}
    .cfi_offset r14, {X86TrapFrame_r14}
    .cfi_offset r13, {X86TrapFrame_r13}
    .cfi_offset r12, {X86TrapFrame_r12}
    .cfi_offset r11, {X86TrapFrame_r11}
    .cfi_offset r10, {X86TrapFrame_r10}
    .cfi_offset r9, {X86TrapFrame_r9}
    .cfi_offset r8, {X86TrapFrame_r8}
    .cfi_offset rdi, {X86TrapFrame_rdi}
    .cfi_offset rsi, {X86TrapFrame_rsi}
    .cfi_offset rbp, {X86TrapFrame_rbp}
    .cfi_offset rdx, {X86TrapFrame_rdx}
    .cfi_offset rcx, {X86TrapFrame_rcx}
    .cfi_offset rbx, {X86TrapFrame_rbx}
    .cfi_offset rax, {X86TrapFrame_rax}
    sub rsp, 8                                              # Reserve space for the fake frame ptr.
    mov rdi, rsp                                            # Set first argument to the trap frame pointer.
    
    cld
    cmp word ptr [rsp+{X86TrapFrame_cs}], {KCODE_SEL}
    je 1f
    swapgs
1:
    
    # Fake kernel unwind frame.
    .cfi_adjust_cfa_offset 8
    mov rax, qword ptr [rsp+{X86TrapFrame_rip}]
    mov rbx, qword ptr [rsp+{X86TrapFrame_rbp}]
    inc rax                                                 # The undwinder biases PC-1; undo that here.
    push rax
    .cfi_adjust_cfa_offset 8
    push rbp
    .cfi_adjust_cfa_offset 8
    mov qword ptr [rdi+{X86TrapFrame_fake_fp}], rsp
    
    call x86_irq_handler
    
    add rsp, 24
    .cfi_adjust_cfa_offset -24
    
    cmp word ptr [rsp+{X86TrapFrame_cs}-8], {KCODE_SEL}     # One of the three qwords just popped is actually in the trap frame.
    je 1f
    swapgs
1:
    
    # Restore GPRs.
    .cfi_restore_state
    pop rax
    .cfi_adjust_cfa_offset -8
    pop rbx
    .cfi_adjust_cfa_offset -8
    pop rcx
    .cfi_adjust_cfa_offset -8
    pop rdx
    .cfi_adjust_cfa_offset -8
    pop rbp
    .cfi_adjust_cfa_offset -8
    pop rsi
    .cfi_adjust_cfa_offset -8
    pop rdi
    .cfi_adjust_cfa_offset -8
    pop r8
    .cfi_adjust_cfa_offset -8
    pop r9
    .cfi_adjust_cfa_offset -8
    pop r10
    .cfi_adjust_cfa_offset -8
    pop r11
    .cfi_adjust_cfa_offset -8
    pop r12
    .cfi_adjust_cfa_offset -8
    pop r13
    .cfi_adjust_cfa_offset -8
    pop r14
    .cfi_adjust_cfa_offset -8
    pop r15
    .cfi_adjust_cfa_offset -8
    
    add rsp, 16 # IRQ numer and error code.
    .cfi_adjust_cfa_offset -16
    iretq
    .cfi_endproc
    .size x86_irq_asm, .-x86_irq_asm
