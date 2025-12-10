
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.10.0)

set(CMAKE_EXPORT_COMPILE_COMMANDS true)
include(../.config/config.cmake)
set(CMAKE_C_COMPILER ${CONFIG_COMPILER})

add_compile_options(
    -static
    -ffreestanding
    -march=${CONFIG_ISA_SPEC}          # Selects the target CPU.
    -mabi=${CONFIG_ABI_SPEC}           # Selects target ABI
    -O2                                # Optimize the code.
    -Werror=return-type                # Error when a function doesn't return a value, but declares to do so.
    -Werror=implicit-fallthrough
    -Wall -Wextra                      # Ramp up warning level.
    -std=gnu11                         # We use the C11 standard
    -DBADGEROS                         # Tell the code we're building for the userspace
    -fno-omit-frame-pointer            # Always use frame pointer
    -fno-stack-protector
    -fno-exceptions
    -ggdb
)
add_link_options(
    -static
    -nodefaultlibs -nostdlib
    -nostartfiles
    -lgcc
)
include_directories(${CMAKE_CURRENT_LIST_DIR}/../kernel/lib/nanoprintf ${CMAKE_CURRENT_LIST_DIR} ${CMAKE_CURRENT_LIST_DIR}/../common/include)

project(root C ASM)

add_executable(init start.S util.c init.c)
add_executable(test2 start.S util.c test2.c)
install(TARGETS init test2 DESTINATION sbin)
