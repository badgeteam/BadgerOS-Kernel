
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.13.0)
cmake_policy(VERSION 3.13.0)

# For IDE users.
set(CMAKE_EXPORT_COMPILE_COMMANDS true)

set(CMAKE_TRY_COMPILE_TARGET_TYPE "STATIC_LIBRARY")

# Set the RAMFS root
if(NOT DEFINED BADGER_RAMFS_ROOT)
    set(BADGER_RAMFS_ROOT ${CMAKE_CURRENT_LIST_DIR}/root)
endif()



# Include the config.
include(../.config/config.cmake)
include(config_helper.cmake)
set(CMAKE_C_COMPILER ${CONFIG_COMPILER})

# Include port-specific.
include(cpu/${CONFIG_CPU}/CMakeLists.txt)



# Define options.
if("${CONFIG_CPU}" STREQUAL "riscv32")
    config_const(BOOT_PROTOCOL "esp32" "Boot protocol")
    config_enum(
        SOC "esp32p4" "Choose chipset"
        "esp32p4" "esp32c6"
    )
    config_const(ENABLE_ACPI false "Enable ACPI support")
    config_bool(ENABLE_DTB true "Enable DTB support")
else()
    config_const(BOOT_PROTOCOL "limine" "Boot protocol")
    config_const(SOC "generic" "Boot protocol")
    if("${CONFIG_CPU}" STREQUAL "x86_64")
        config_const(ENABLE_ACPI true "Enable ACPI support")
        config_const(ENABLE_DTB false "Enable DTB support")
    else()
        config_bool(ENABLE_ACPI true "Enable ACPI support")
        config_bool(ENABLE_DTB true "Enable DTB support")
    endif()
endif()

config_enum(
    STACK_SIZE 16384 "Kernel stack size"
    8192 16384 32768 65536 131072 262144
)
config_enum(
    TICKS_PER_SEC 500 "How many tick interrupts happen per second"
    100 200 250 500 1000
)
config_enum(
    MAX_CPUS 32 "Maximum number of usable CPUs (more will be ignored)"
    32 64 128 256
)
config_enum(
    LOAD_MEASURE_WINDOW 200 "How many ticks to average over for load CPU measurement"
    25 50 100 200 500 1000
)

if("${CONFIG_BOOT_PROTOCOL}" STREQUAL "limine" AND "${CONFIG_CPU}" STREQUAL "riscv64")
    config_bool(EMBED_UBOOT true "Embed U-boot")
else()
    config_const(EMBED_UBOOT false "Embed U-boot")
endif()

config_const(NOMMU false "Build for system without an MMU")
config_const(PAGE_SIZE 4096 "MMU / memory map page size")

config_bool(KTEST false "Run kernel self tests")



set(common_compiler_flags
    -ffreestanding -DCHAR_BIT=8        # We do not compile against an OS.
    -march=${CONFIG_KISA_SPEC}         # Selects the target CPU.
    -mabi=${CONFIG_KABI_SPEC}          # Selects target ABI
    -nodefaultlibs                     # Do not link any default libraries like libgcc or libc.
    -ggdb -gdwarf-2                    # Generate debug information in default extended format.
    -Werror=return-type                # Error when a function doesn't return a value, but declares to do so.
    -Werror=implicit-fallthrough
    -Werror=int-conversion
    -Werror=incompatible-pointer-types
    -Werror=implicit-function-declaration
    -Wall -Wextra                      # Ramp up warning level.
    -Wno-missing-braces
    -std=gnu11                         # We use the C11 standard
    -DBADGEROS_KERNEL=1                # Tell the code we're building for the kernel
    -DBADGEROS_MALLOC_DEBUG_LEVEL=2    # Malloc debug level set to WARN
    -DSOFTBIT                          # Turn on our emulated bit operations
    -fno-omit-frame-pointer            # Always use frame pointer
    -static
    -ffunction-sections
    -fno-stack-protector
    -fno-exceptions
    -Wno-unused-parameter
)
if(CMAKE_BUILD_TYPE STREQUAL "Release")
    message("Building in release mode")
    list(APPEND common_compiler_flags -O2)
else()
    message("Building in debug mode")
    list(APPEND common_compiler_flags -O0)
endif()

# we must pass the same options to GCC and LD when using LTO, as the linker will actually do the codegen
add_compile_options(${common_compiler_flags} ${cpu_flags})
add_link_options(
    ${common_compiler_flags} ${cpu_flags} ${cpu_link}
    -Wl,--gc-sections -Wl,--build-id=none -nodefaultlibs -nostartfiles
    -T${CMAKE_CURRENT_LIST_DIR}/misc/ld/${CONFIG_SOC}_${CONFIG_CPU}.ld
)



# Define executable.
project(badgeros C ASM)
set(target badger-os.elf)

set(kernel_src
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/arrays.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/badge_format_str.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/badge_strings.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/fifo.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/hash.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/list.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/log.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/lstr.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/map.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/num_to_str.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/radixtree.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/rawprint.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/refcount.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/set.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/spinlock.c
    ${CMAKE_CURRENT_LIST_DIR}/src/badgelib/nanoprintf.c
    
    ${CMAKE_CURRENT_LIST_DIR}/src/boot/${CONFIG_BOOT_PROTOCOL}.c
    
    ${CMAKE_CURRENT_LIST_DIR}/src/device/builtin_driver/pcie.c
    ${CMAKE_CURRENT_LIST_DIR}/src/device/class/block.c
    ${CMAKE_CURRENT_LIST_DIR}/src/device/class/char.c
    ${CMAKE_CURRENT_LIST_DIR}/src/device/class/pcictl.c
    ${CMAKE_CURRENT_LIST_DIR}/src/device/class/tty.c
    ${CMAKE_CURRENT_LIST_DIR}/src/device/builtin_drivers.c
    ${CMAKE_CURRENT_LIST_DIR}/src/device/device.c
    
    ${CMAKE_CURRENT_LIST_DIR}/src/freestanding/int_routines.c
    ${CMAKE_CURRENT_LIST_DIR}/src/freestanding/string.c
    
    ${CMAKE_CURRENT_LIST_DIR}/src/malloc/malloc.c
    ${CMAKE_CURRENT_LIST_DIR}/src/malloc/static-buddy-compat.c
    ${CMAKE_CURRENT_LIST_DIR}/src/malloc/slab-alloc.c
    
    ${CMAKE_CURRENT_LIST_DIR}/src/mem/mod.c
    
    ${CMAKE_CURRENT_LIST_DIR}/src/errno.c
    ${CMAKE_CURRENT_LIST_DIR}/src/page_alloc.c
    ${CMAKE_CURRENT_LIST_DIR}/src/time.c
)
set(kernel_include
    ${CMAKE_CURRENT_LIST_DIR}/lib/nanoprintf
    ${CMAKE_CURRENT_LIST_DIR}/include
    ${CMAKE_CURRENT_LIST_DIR}/include/badgelib
    ${CMAKE_CURRENT_LIST_DIR}/../.config
)

if(${CONFIG_ENABLE_ACPI})
    include(lib/uacpi/uacpi.cmake)
    set(kernel_src ${kernel_src}
        ${CMAKE_CURRENT_LIST_DIR}/src/device/uacpi_kernel_api.c
        ${UACPI_SOURCES}
    )
    set(kernel_include ${kernel_include} ${UACPI_INCLUDES})
endif()

if(${CONFIG_ENABLE_DTB})
    include(lib/uacpi/uacpi.cmake)
    set(kernel_src ${kernel_src}
        ${CMAKE_CURRENT_LIST_DIR}/src/device/dtb/dtb.c
        ${CMAKE_CURRENT_LIST_DIR}/src/device/dtb/dtparse.c
    )
    set(kernel_include ${kernel_include} ${UACPI_INCLUDES})
endif()

add_executable(${target}
    ${kernel_src}
    ${cpu_src}
)
include_directories(
    ${kernel_include}
    ${cpu_include}
)

# Export configs in a way visible to Rust.
set(RUST_CONFIG_FILE "")
foreach(config_name ${ALL_CONFIGS})
    set(RUST_CONFIG_FILE "${RUST_CONFIG_FILE}cmake_config!(${config_name}, ${CONFIG_${config_name}});")
endforeach()
file(CONFIGURE OUTPUT "cmake_config.rs" CONTENT "${RUST_CONFIG_FILE}")

# Integrate Rust.
set(rust_features)
if(${CONFIG_KTEST})
    set(rust_features ${rust_features} --features=ktest)
endif()
if(${CONFIG_ENABLE_ACPI})
    set(rust_features ${rust_features} --features=acpi)
endif()
if(${CONFIG_ENABLE_DTB})
    set(rust_features ${rust_features} --features=dtb)
endif()
if(CMAKE_BUILD_TYPE STREQUAL "Release")
    set(rust_features ${rust_features} --release)
    set(rust_build_type release)
else()
    set(rust_build_type debug)
endif()
set(rust_target ${CMAKE_CURRENT_LIST_DIR}/misc/rust_target/${CONFIG_CPU}-kernel.json)
set(rust_lib ${CMAKE_CURRENT_LIST_DIR}/target/${CONFIG_CPU}-kernel/${rust_build_type}/libkernel.a)
add_custom_target(
    rust.target ALL
    cargo build ${rust_features} --target=${rust_target}
)
add_dependencies(${target} rust.target)
target_link_libraries(${target} PRIVATE -Wl,--whole-archive ${rust_lib} -Wl,--no-whole-archive)



# Copy the BadgerOS binary to the output directory.
install(TARGETS badger-os.elf DESTINATION .)
add_custom_command(
    OUTPUT badger-os.stripped.elf
    ALL
    COMMAND cp badger-os.elf badger-os.stripped.elf
    COMMAND ${CONFIG_PREFIX}strip -g -s badger-os.stripped.elf
    DEPENDS badger-os.elf
)
install(FILES ${CMAKE_BINARY_DIR}/badger-os.stripped.elf DESTINATION .)

# Create a disassembled version for easier debugging.
add_custom_target(
    badger-os.elf.disasm
    ALL
    COMMAND ${CONFIG_PREFIX}objdump -Sd badger-os.elf > badger-os.elf.disasm
    DEPENDS badger-os.elf
)

if("${CONFIG_BOOT_PROTOCOL}" STREQUAL "esp32")
    include(image_esp32.cmake)
elseif("${CONFIG_BOOT_PROTOCOL}" STREQUAL "limine")
    include(image_limine.cmake)
endif()
