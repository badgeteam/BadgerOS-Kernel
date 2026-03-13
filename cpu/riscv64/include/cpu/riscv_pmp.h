
// SPDX-License-Identifier: MIT

#pragma once

// This file expects RISCV_PMP_REGION_COUNT to be defined by "port/hardware.h".

#include "assertions.h"
#include "attributes.h"
#include "cpu/riscv.h"
#include "meta.h"
#include "port/hardware.h"
#include "port/hardware_allocation.h"

#include <stddef.h>
#include <stdint.h>

typedef struct proc_memmap_t proc_memmap_t;

static_assert(
    RISCV_PMP_REGION_COUNT == 16 || RISCV_PMP_REGION_COUNT == 64, "RISC-V PMP region count must be either 16 or 64."
);

#if __riscv_xlen == 32

// 2^XLEN-byte NAPOT range starting at address 0.
#define RISCV_PMPADDR_NAPOT_GLOBAL __LONG_MAX__

#define __RISCV_PMP_CFG_BITPOS_0  0
#define __RISCV_PMP_CFG_BITPOS_1  8
#define __RISCV_PMP_CFG_BITPOS_2  16
#define __RISCV_PMP_CFG_BITPOS_3  24
#define __RISCV_PMP_CFG_BITPOS_4  0
#define __RISCV_PMP_CFG_BITPOS_5  8
#define __RISCV_PMP_CFG_BITPOS_6  16
#define __RISCV_PMP_CFG_BITPOS_7  24
#define __RISCV_PMP_CFG_BITPOS_8  0
#define __RISCV_PMP_CFG_BITPOS_9  8
#define __RISCV_PMP_CFG_BITPOS_10 16
#define __RISCV_PMP_CFG_BITPOS_11 24
#define __RISCV_PMP_CFG_BITPOS_12 0
#define __RISCV_PMP_CFG_BITPOS_13 8
#define __RISCV_PMP_CFG_BITPOS_14 16
#define __RISCV_PMP_CFG_BITPOS_15 24
#if RISCV_PMP_REGION_COUNT == 64
#define __RISCV_PMP_CFG_BITPOS_16 0
#define __RISCV_PMP_CFG_BITPOS_17 8
#define __RISCV_PMP_CFG_BITPOS_18 16
#define __RISCV_PMP_CFG_BITPOS_19 24
#define __RISCV_PMP_CFG_BITPOS_20 0
#define __RISCV_PMP_CFG_BITPOS_21 8
#define __RISCV_PMP_CFG_BITPOS_22 16
#define __RISCV_PMP_CFG_BITPOS_23 24
#define __RISCV_PMP_CFG_BITPOS_24 0
#define __RISCV_PMP_CFG_BITPOS_25 8
#define __RISCV_PMP_CFG_BITPOS_26 16
#define __RISCV_PMP_CFG_BITPOS_27 24
#define __RISCV_PMP_CFG_BITPOS_28 0
#define __RISCV_PMP_CFG_BITPOS_29 8
#define __RISCV_PMP_CFG_BITPOS_30 16
#define __RISCV_PMP_CFG_BITPOS_31 24
#define __RISCV_PMP_CFG_BITPOS_32 0
#define __RISCV_PMP_CFG_BITPOS_33 8
#define __RISCV_PMP_CFG_BITPOS_34 16
#define __RISCV_PMP_CFG_BITPOS_35 24
#define __RISCV_PMP_CFG_BITPOS_36 0
#define __RISCV_PMP_CFG_BITPOS_37 8
#define __RISCV_PMP_CFG_BITPOS_38 16
#define __RISCV_PMP_CFG_BITPOS_39 24
#define __RISCV_PMP_CFG_BITPOS_40 0
#define __RISCV_PMP_CFG_BITPOS_41 8
#define __RISCV_PMP_CFG_BITPOS_42 16
#define __RISCV_PMP_CFG_BITPOS_43 24
#define __RISCV_PMP_CFG_BITPOS_44 0
#define __RISCV_PMP_CFG_BITPOS_45 8
#define __RISCV_PMP_CFG_BITPOS_46 16
#define __RISCV_PMP_CFG_BITPOS_47 24
#define __RISCV_PMP_CFG_BITPOS_48 0
#define __RISCV_PMP_CFG_BITPOS_49 8
#define __RISCV_PMP_CFG_BITPOS_50 16
#define __RISCV_PMP_CFG_BITPOS_51 24
#define __RISCV_PMP_CFG_BITPOS_52 0
#define __RISCV_PMP_CFG_BITPOS_53 8
#define __RISCV_PMP_CFG_BITPOS_54 16
#define __RISCV_PMP_CFG_BITPOS_55 24
#define __RISCV_PMP_CFG_BITPOS_56 0
#define __RISCV_PMP_CFG_BITPOS_57 8
#define __RISCV_PMP_CFG_BITPOS_58 16
#define __RISCV_PMP_CFG_BITPOS_59 24
#define __RISCV_PMP_CFG_BITPOS_60 0
#define __RISCV_PMP_CFG_BITPOS_61 8
#define __RISCV_PMP_CFG_BITPOS_62 16
#define __RISCV_PMP_CFG_BITPOS_63 24
#endif

#define __RISCV_PMP_CFG_CSR_0  "pmpcfg0"
#define __RISCV_PMP_CFG_CSR_1  "pmpcfg0"
#define __RISCV_PMP_CFG_CSR_2  "pmpcfg0"
#define __RISCV_PMP_CFG_CSR_3  "pmpcfg0"
#define __RISCV_PMP_CFG_CSR_4  "pmpcfg1"
#define __RISCV_PMP_CFG_CSR_5  "pmpcfg1"
#define __RISCV_PMP_CFG_CSR_6  "pmpcfg1"
#define __RISCV_PMP_CFG_CSR_7  "pmpcfg1"
#define __RISCV_PMP_CFG_CSR_8  "pmpcfg2"
#define __RISCV_PMP_CFG_CSR_9  "pmpcfg2"
#define __RISCV_PMP_CFG_CSR_10 "pmpcfg2"
#define __RISCV_PMP_CFG_CSR_11 "pmpcfg2"
#define __RISCV_PMP_CFG_CSR_12 "pmpcfg3"
#define __RISCV_PMP_CFG_CSR_13 "pmpcfg3"
#define __RISCV_PMP_CFG_CSR_14 "pmpcfg3"
#define __RISCV_PMP_CFG_CSR_15 "pmpcfg3"
#if RISCV_PMP_REGION_COUNT == 64
#define __RISCV_PMP_CFG_CSR_16 "pmpcfg4"
#define __RISCV_PMP_CFG_CSR_17 "pmpcfg4"
#define __RISCV_PMP_CFG_CSR_18 "pmpcfg4"
#define __RISCV_PMP_CFG_CSR_19 "pmpcfg4"
#define __RISCV_PMP_CFG_CSR_20 "pmpcfg5"
#define __RISCV_PMP_CFG_CSR_21 "pmpcfg5"
#define __RISCV_PMP_CFG_CSR_22 "pmpcfg5"
#define __RISCV_PMP_CFG_CSR_23 "pmpcfg5"
#define __RISCV_PMP_CFG_CSR_24 "pmpcfg6"
#define __RISCV_PMP_CFG_CSR_25 "pmpcfg6"
#define __RISCV_PMP_CFG_CSR_26 "pmpcfg6"
#define __RISCV_PMP_CFG_CSR_27 "pmpcfg6"
#define __RISCV_PMP_CFG_CSR_28 "pmpcfg7"
#define __RISCV_PMP_CFG_CSR_29 "pmpcfg7"
#define __RISCV_PMP_CFG_CSR_30 "pmpcfg7"
#define __RISCV_PMP_CFG_CSR_31 "pmpcfg7"
#define __RISCV_PMP_CFG_CSR_32 "pmpcfg8"
#define __RISCV_PMP_CFG_CSR_33 "pmpcfg8"
#define __RISCV_PMP_CFG_CSR_34 "pmpcfg8"
#define __RISCV_PMP_CFG_CSR_35 "pmpcfg8"
#define __RISCV_PMP_CFG_CSR_36 "pmpcfg9"
#define __RISCV_PMP_CFG_CSR_37 "pmpcfg9"
#define __RISCV_PMP_CFG_CSR_38 "pmpcfg9"
#define __RISCV_PMP_CFG_CSR_39 "pmpcfg9"
#define __RISCV_PMP_CFG_CSR_40 "pmpcfg10"
#define __RISCV_PMP_CFG_CSR_41 "pmpcfg10"
#define __RISCV_PMP_CFG_CSR_42 "pmpcfg10"
#define __RISCV_PMP_CFG_CSR_43 "pmpcfg10"
#define __RISCV_PMP_CFG_CSR_44 "pmpcfg11"
#define __RISCV_PMP_CFG_CSR_45 "pmpcfg11"
#define __RISCV_PMP_CFG_CSR_46 "pmpcfg11"
#define __RISCV_PMP_CFG_CSR_47 "pmpcfg11"
#define __RISCV_PMP_CFG_CSR_48 "pmpcfg12"
#define __RISCV_PMP_CFG_CSR_49 "pmpcfg12"
#define __RISCV_PMP_CFG_CSR_50 "pmpcfg12"
#define __RISCV_PMP_CFG_CSR_51 "pmpcfg12"
#define __RISCV_PMP_CFG_CSR_52 "pmpcfg13"
#define __RISCV_PMP_CFG_CSR_53 "pmpcfg13"
#define __RISCV_PMP_CFG_CSR_54 "pmpcfg13"
#define __RISCV_PMP_CFG_CSR_55 "pmpcfg13"
#define __RISCV_PMP_CFG_CSR_56 "pmpcfg14"
#define __RISCV_PMP_CFG_CSR_57 "pmpcfg14"
#define __RISCV_PMP_CFG_CSR_58 "pmpcfg14"
#define __RISCV_PMP_CFG_CSR_59 "pmpcfg14"
#define __RISCV_PMP_CFG_CSR_60 "pmpcfg15"
#define __RISCV_PMP_CFG_CSR_61 "pmpcfg15"
#define __RISCV_PMP_CFG_CSR_62 "pmpcfg15"
#define __RISCV_PMP_CFG_CSR_63 "pmpcfg15"
#endif

#elif __riscv_xlen == 64

// 2^XLEN-byte NAPOT range starting at address 0.
#define RISCV_PMPADDR_NAPOT_GLOBAL 0x1fffffffffffffff

#define __RISCV_PMP_CFG_BITPOS_0  0
#define __RISCV_PMP_CFG_BITPOS_1  8
#define __RISCV_PMP_CFG_BITPOS_2  16
#define __RISCV_PMP_CFG_BITPOS_3  24
#define __RISCV_PMP_CFG_BITPOS_4  32
#define __RISCV_PMP_CFG_BITPOS_5  40
#define __RISCV_PMP_CFG_BITPOS_6  48
#define __RISCV_PMP_CFG_BITPOS_7  56
#define __RISCV_PMP_CFG_BITPOS_8  0
#define __RISCV_PMP_CFG_BITPOS_9  8
#define __RISCV_PMP_CFG_BITPOS_10 16
#define __RISCV_PMP_CFG_BITPOS_11 24
#define __RISCV_PMP_CFG_BITPOS_12 32
#define __RISCV_PMP_CFG_BITPOS_13 40
#define __RISCV_PMP_CFG_BITPOS_14 48
#define __RISCV_PMP_CFG_BITPOS_15 56
#if RISCV_PMP_REGION_COUNT == 64
#define __RISCV_PMP_CFG_BITPOS_16 0
#define __RISCV_PMP_CFG_BITPOS_17 8
#define __RISCV_PMP_CFG_BITPOS_18 16
#define __RISCV_PMP_CFG_BITPOS_19 24
#define __RISCV_PMP_CFG_BITPOS_20 32
#define __RISCV_PMP_CFG_BITPOS_21 40
#define __RISCV_PMP_CFG_BITPOS_22 48
#define __RISCV_PMP_CFG_BITPOS_23 56
#define __RISCV_PMP_CFG_BITPOS_24 0
#define __RISCV_PMP_CFG_BITPOS_25 8
#define __RISCV_PMP_CFG_BITPOS_26 16
#define __RISCV_PMP_CFG_BITPOS_27 24
#define __RISCV_PMP_CFG_BITPOS_28 32
#define __RISCV_PMP_CFG_BITPOS_29 40
#define __RISCV_PMP_CFG_BITPOS_30 48
#define __RISCV_PMP_CFG_BITPOS_31 56
#define __RISCV_PMP_CFG_BITPOS_32 0
#define __RISCV_PMP_CFG_BITPOS_33 8
#define __RISCV_PMP_CFG_BITPOS_34 16
#define __RISCV_PMP_CFG_BITPOS_35 24
#define __RISCV_PMP_CFG_BITPOS_36 32
#define __RISCV_PMP_CFG_BITPOS_37 40
#define __RISCV_PMP_CFG_BITPOS_38 48
#define __RISCV_PMP_CFG_BITPOS_39 56
#define __RISCV_PMP_CFG_BITPOS_40 0
#define __RISCV_PMP_CFG_BITPOS_41 8
#define __RISCV_PMP_CFG_BITPOS_42 16
#define __RISCV_PMP_CFG_BITPOS_43 24
#define __RISCV_PMP_CFG_BITPOS_44 32
#define __RISCV_PMP_CFG_BITPOS_45 40
#define __RISCV_PMP_CFG_BITPOS_46 48
#define __RISCV_PMP_CFG_BITPOS_47 56
#define __RISCV_PMP_CFG_BITPOS_48 0
#define __RISCV_PMP_CFG_BITPOS_49 8
#define __RISCV_PMP_CFG_BITPOS_50 16
#define __RISCV_PMP_CFG_BITPOS_51 24
#define __RISCV_PMP_CFG_BITPOS_52 32
#define __RISCV_PMP_CFG_BITPOS_53 40
#define __RISCV_PMP_CFG_BITPOS_54 48
#define __RISCV_PMP_CFG_BITPOS_55 56
#define __RISCV_PMP_CFG_BITPOS_56 0
#define __RISCV_PMP_CFG_BITPOS_57 8
#define __RISCV_PMP_CFG_BITPOS_58 16
#define __RISCV_PMP_CFG_BITPOS_59 24
#define __RISCV_PMP_CFG_BITPOS_60 32
#define __RISCV_PMP_CFG_BITPOS_61 40
#define __RISCV_PMP_CFG_BITPOS_62 48
#define __RISCV_PMP_CFG_BITPOS_63 56
#endif

#define __RISCV_PMP_CFG_CSR_0  "pmpcfg0"
#define __RISCV_PMP_CFG_CSR_1  "pmpcfg0"
#define __RISCV_PMP_CFG_CSR_2  "pmpcfg0"
#define __RISCV_PMP_CFG_CSR_3  "pmpcfg0"
#define __RISCV_PMP_CFG_CSR_4  "pmpcfg0"
#define __RISCV_PMP_CFG_CSR_5  "pmpcfg0"
#define __RISCV_PMP_CFG_CSR_6  "pmpcfg0"
#define __RISCV_PMP_CFG_CSR_7  "pmpcfg0"
#define __RISCV_PMP_CFG_CSR_8  "pmpcfg2"
#define __RISCV_PMP_CFG_CSR_9  "pmpcfg2"
#define __RISCV_PMP_CFG_CSR_10 "pmpcfg2"
#define __RISCV_PMP_CFG_CSR_11 "pmpcfg2"
#define __RISCV_PMP_CFG_CSR_12 "pmpcfg2"
#define __RISCV_PMP_CFG_CSR_13 "pmpcfg2"
#define __RISCV_PMP_CFG_CSR_14 "pmpcfg2"
#define __RISCV_PMP_CFG_CSR_15 "pmpcfg2"
#if RISCV_PMP_REGION_COUNT == 64
#define __RISCV_PMP_CFG_CSR_16 "pmpcfg4"
#define __RISCV_PMP_CFG_CSR_17 "pmpcfg4"
#define __RISCV_PMP_CFG_CSR_18 "pmpcfg4"
#define __RISCV_PMP_CFG_CSR_19 "pmpcfg4"
#define __RISCV_PMP_CFG_CSR_20 "pmpcfg4"
#define __RISCV_PMP_CFG_CSR_21 "pmpcfg4"
#define __RISCV_PMP_CFG_CSR_22 "pmpcfg4"
#define __RISCV_PMP_CFG_CSR_23 "pmpcfg4"
#define __RISCV_PMP_CFG_CSR_24 "pmpcfg6"
#define __RISCV_PMP_CFG_CSR_25 "pmpcfg6"
#define __RISCV_PMP_CFG_CSR_26 "pmpcfg6"
#define __RISCV_PMP_CFG_CSR_27 "pmpcfg6"
#define __RISCV_PMP_CFG_CSR_28 "pmpcfg6"
#define __RISCV_PMP_CFG_CSR_29 "pmpcfg6"
#define __RISCV_PMP_CFG_CSR_30 "pmpcfg6"
#define __RISCV_PMP_CFG_CSR_31 "pmpcfg6"
#define __RISCV_PMP_CFG_CSR_32 "pmpcfg8"
#define __RISCV_PMP_CFG_CSR_33 "pmpcfg8"
#define __RISCV_PMP_CFG_CSR_34 "pmpcfg8"
#define __RISCV_PMP_CFG_CSR_35 "pmpcfg8"
#define __RISCV_PMP_CFG_CSR_36 "pmpcfg8"
#define __RISCV_PMP_CFG_CSR_37 "pmpcfg8"
#define __RISCV_PMP_CFG_CSR_38 "pmpcfg8"
#define __RISCV_PMP_CFG_CSR_39 "pmpcfg8"
#define __RISCV_PMP_CFG_CSR_40 "pmpcfg10"
#define __RISCV_PMP_CFG_CSR_41 "pmpcfg10"
#define __RISCV_PMP_CFG_CSR_42 "pmpcfg10"
#define __RISCV_PMP_CFG_CSR_43 "pmpcfg10"
#define __RISCV_PMP_CFG_CSR_44 "pmpcfg10"
#define __RISCV_PMP_CFG_CSR_45 "pmpcfg10"
#define __RISCV_PMP_CFG_CSR_46 "pmpcfg10"
#define __RISCV_PMP_CFG_CSR_47 "pmpcfg10"
#define __RISCV_PMP_CFG_CSR_48 "pmpcfg12"
#define __RISCV_PMP_CFG_CSR_49 "pmpcfg12"
#define __RISCV_PMP_CFG_CSR_50 "pmpcfg12"
#define __RISCV_PMP_CFG_CSR_51 "pmpcfg12"
#define __RISCV_PMP_CFG_CSR_52 "pmpcfg12"
#define __RISCV_PMP_CFG_CSR_53 "pmpcfg12"
#define __RISCV_PMP_CFG_CSR_54 "pmpcfg12"
#define __RISCV_PMP_CFG_CSR_55 "pmpcfg12"
#define __RISCV_PMP_CFG_CSR_56 "pmpcfg14"
#define __RISCV_PMP_CFG_CSR_57 "pmpcfg14"
#define __RISCV_PMP_CFG_CSR_58 "pmpcfg14"
#define __RISCV_PMP_CFG_CSR_59 "pmpcfg14"
#define __RISCV_PMP_CFG_CSR_60 "pmpcfg14"
#define __RISCV_PMP_CFG_CSR_61 "pmpcfg14"
#define __RISCV_PMP_CFG_CSR_62 "pmpcfg14"
#define __RISCV_PMP_CFG_CSR_63 "pmpcfg14"
#endif

#endif



// RISC-V PMP addressing mode.
typedef enum {
    // Disabled.
    RISCV_PMPCFG_OFF   = 0,
    // Top of range.
    RISCV_PMPCFG_TOR   = 1,
    // Naturally aligned 4-byte region.
    RISCV_PMPCFG_NA4   = 2,
    // Naturally aligned power-of-two region.
    RISCV_PMPCFG_NAPOT = 3,
} riscv_pmpcfg_addr_t;

// RISC-V PMP config entry.
typedef union {
    struct {
        // Read permission granted.
        uint8_t read  : 1;
        // Write permission granted.
        uint8_t write : 1;
        // Execute permission granted.
        uint8_t exec  : 1;

        // Address matching mode.
        uint8_t addr_match_mode : 2;
        // Reserved; set to 0.
        uint8_t                 : 2;
        // Lock and apply to M-mode accesses.
        uint8_t lock            : 1;
    };
    uint8_t value;
} riscv_pmpcfg_t;

typedef struct {
    // PMP configurations.
    riscv_pmpcfg_t pmpcfg[PROC_RISCV_PMP_COUNT];
    // PMP addresses.
    size_t         pmpaddr[PROC_RISCV_PMP_COUNT];
} riscv_pmp_ctx_t;



// Initialise memory protection driver.
void riscv_pmp_init();

// Read all raw PMP configurations.
void riscv_pmpcfg_read_all(riscv_pmpcfg_t cfg_out[RISCV_PMP_REGION_COUNT]);
// Read all raw PMP addresses.
void riscv_pmpaddr_read_all(size_t addr_out[RISCV_PMP_REGION_COUNT]);
// Write all raw PMP configurations.
void riscv_pmpcfg_write_all(riscv_pmpcfg_t const cfg_in[RISCV_PMP_REGION_COUNT]);
// Write all raw PMP addresses.
void riscv_pmpaddr_write_all(size_t const addr_in[RISCV_PMP_REGION_COUNT]);

// Add a memory protection region.
// For kernels using PMP as memory protection.
bool riscv_pmp_memprotect(proc_memmap_t *new_mm, riscv_pmp_ctx_t *ctx, size_t paddr, size_t length, uint32_t flags);
// Swap in the set of PMP entries.
void riscv_pmp_memprotect_swap(riscv_pmp_ctx_t *ctx);



// Determine whether an address is a valid NAPOT address.
static inline bool riscv_pmpaddr_is_napot(size_t base, size_t size) CONST;
static inline bool riscv_pmpaddr_is_napot(size_t base, size_t size) {
    // Size must be a power of 2 >= 8.
    if (size < 8 || (size & (size - 1))) {
        return false;
    }
    // Base must be aligned to size.
    return (base & (size - 1)) == 0;
}

// Compute NAPOT address value.
// Assumes `base` is aligned to `size` bytes and that `size` is a power of two >= 8.
static inline size_t riscv_pmpaddr_calc_napot(size_t base, size_t size) CONST;
static inline size_t riscv_pmpaddr_calc_napot(size_t base, size_t size) {
    return (base >> 2) | ((size >> 3) - 1);
}

// Clear bits of a PMPCFG by index.
#define riscv_pmpcfg_clear(index)                                                                                      \
    asm volatile("csrc " concat_keywords(__RISCV_PMP_CFG_CSR_, index) ", %0" ::"r"(                                    \
        255ul << concat_keywords(__RISCV_PMP_CFG_BITPOS_, index)                                                       \
    ))

// Set bits of a PMPCFG by index.
#define riscv_pmpcfg_set(index, config)                                                                                \
    asm volatile("csrs " concat_keywords(__RISCV_PMP_CFG_CSR_, index) ", %0" ::"r"(                                    \
        (unsigned long)(config).value << (unsigned long)concat_keywords(__RISCV_PMP_CFG_BITPOS_, index)                \
    ))

// Write a PMPCFG by index.
#define riscv_pmpcfg_write(index, config)                                                                              \
    do {                                                                                                               \
        riscv_pmpcfg_clear(index);                                                                                     \
        riscv_pmpcfg_set(index, config);                                                                               \
    } while (0)

// Write a PMPADDR by index.
#define riscv_pmpaddr_write(index, addr) asm volatile("csrw pmpaddr" convert_macro_to_string(index) ", %0" ::"r"(addr))
