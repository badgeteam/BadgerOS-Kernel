
# SPDX-License-Identifier: MIT



    # Entrypoint from the bootloader.
    .text
    .align 2
    .global _start
    .type _start, %function
    .cfi_startproc
_start:
    .cfi_undefined %sp
    .cfi_undefined %fp
    .cfi_undefined %ra
    # Set up registers.
    .option push
    .option norelax
    la   gp, __global_pointer$
    .option pop
    mv   tp, x0
    
#ifdef CONFIG_BOOT_PROTOCOL_ESP32
    # Zero out .bss section.
    la   a0, __start_bss
    la   a1, __stop_bss
    beq  a0, a1, .bssinit_skip
.bssinit_loop:
    sw   x0, 0(a0)
    addi a0, a0, 4
    bne  a0, a1, .bssinit_loop
.bssinit_skip:
#endif
    
    # Jump to the C entrypoint.
    la   ra, basic_runtime_init
    jalr ra
    # This function isn't allowed to return.
    ebreak
    .cfi_endproc
    .size _start, .-_start
