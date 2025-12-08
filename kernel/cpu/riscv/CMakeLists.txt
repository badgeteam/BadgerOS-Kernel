
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.10.0)

set(cpu_src
    ${CMAKE_CURRENT_LIST_DIR}/src/driver/riscv_intc.c
    ${CMAKE_CURRENT_LIST_DIR}/src/driver/riscv_plic.c
    
    ${CMAKE_CURRENT_LIST_DIR}/src/entrypoint.S
    ${CMAKE_CURRENT_LIST_DIR}/src/isr.S
    ${CMAKE_CURRENT_LIST_DIR}/src/isr_ctx.S
    ${CMAKE_CURRENT_LIST_DIR}/src/syscall.S

    ${CMAKE_CURRENT_LIST_DIR}/src/backtrace.c
    ${CMAKE_CURRENT_LIST_DIR}/src/interrupt.c
    ${CMAKE_CURRENT_LIST_DIR}/src/isr.c
    ${CMAKE_CURRENT_LIST_DIR}/src/isr_ctx.c
    ${CMAKE_CURRENT_LIST_DIR}/src/kmodule.c
    ${CMAKE_CURRENT_LIST_DIR}/src/panic.c
    ${CMAKE_CURRENT_LIST_DIR}/src/sbi_time.c
    ${CMAKE_CURRENT_LIST_DIR}/src/scheduler.c
    # ${CMAKE_CURRENT_LIST_DIR}/src/usercopy.c
)
set(cpu_include ${CMAKE_CURRENT_LIST_DIR}/include)
set(cpu_link)
set(cpu_flags)
set(kbelf_target riscv)

# if(CONFIG_NOMMU)
#     set(cpu_src ${cpu_src} ${CMAKE_CURRENT_LIST_DIR}/src/memprotect/riscv_pmp.c)
# else()
#     set(cpu_src ${cpu_src} ${CMAKE_CURRENT_LIST_DIR}/src/memprotect/mmu.c)
# endif()
