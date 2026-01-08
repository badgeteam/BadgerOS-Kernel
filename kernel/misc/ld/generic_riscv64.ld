OUTPUT_FORMAT(elf64-littleriscv)
OUTPUT_ARCH(riscv)

ENTRY(_start)

PHDRS
{
    text     PT_LOAD    FLAGS(0x05); /* Execute + Read */
    rodata   PT_LOAD    FLAGS(0x04); /* Read only */
    data     PT_LOAD    FLAGS(0x06); /* Write + Read */
    dynamic  PT_DYNAMIC FLAGS(0x06); /* Dynamic PHDR for relocations */
}

SECTIONS
{
    . = 0xffffffff80000000;
    __start_badgeros_kernel = .;

    __start_text = .;
    . = ALIGN(256);
    .interrupt_vector_table : {
        *(.interrupt_vector_table)
    } :text
    .text : {
        *(.text.riscv_exception_asm)
        *(.text.__trap_asm)
        *(.text .text.*)
    } :text
    . = ALIGN(4096);
    __stop_text = .;

    __start_rodata = .;
    .rodata : {
        *(.rodata .rodata.* .srodata .srodata.*)
        . = ALIGN(16);
        __start_noexc = .;
        KEEP(*(.noexc_table .noexc_table.*))
        __stop_noexc = .;
        __start_kmodules = .;
        KEEP(*(.kmodules .kmodules.*))
        __stop_kmodules = .;
        __start_ktests = .;
        KEEP(*(.ktests .ktests.*))
        __stop_ktests = .;
    } :rodata
    .dynstr : { *(.dynstr) } :rodata
    .dynsym : { *(.dynsym) } :rodata
    .rela.dyn : { *(.rela.dyn) } :rodata
    . = ALIGN(4096);
    __stop_rodata = .;

    __start_data = .;
    .dynamic : { *(.dynamic) } :data :dynamic
    .got : { *(.got) } :data
    .got.plt : { *(.got.plt) } :data
    .data : {
        /* Limine requests. */
        KEEP(*(.requests_start))
        KEEP(*(.requests))
        KEEP(*(.requests_end))
        *(.data .data.*)
        *(.sdata .sdata.*)
    } :data
    PROVIDE(__global_pointer$ = .);

    /* Add zero-initialized memory last to avoid padding. */
    .bss : {
        *(.sbss .sbss.*)
        *(.bss .bss.*)
        *(COMMON)
    } :data
    . = ALIGN(4096);
    __stop_data = .;

    __stop_badgeros_kernel = .;

    /* Discard useless sections. */
    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
        *(.interp)
    }
}