
// SPDX-License-Identifier: MIT

#pragma once

#ifndef __ASSEMBLER__
#include "attributes.h"

#include <stdint.h>
#endif



#ifndef __ASSEMBLER__
// Format of EFER MSR.
typedef union {
    struct {
        // System call extensions.
        uint64_t sce   : 1;
        // Reserved.
        uint64_t       : 7;
        // Long mode enable.
        uint64_t lme   : 1;
        // Long mode active.
        uint64_t lma   : 1;
        // No-execute enable.
        uint64_t nxe   : 1;
        // Secure virtual machine enable.
        uint64_t svme  : 1;
        // Fast FXSAVE/FXSTOR.
        uint64_t ffxsr : 1;
        // Translation cache extension.
        uint64_t tce   : 1;
    };
    uint64_t val;
} msr_efer_t;
#endif



// Address of FSBASE MSR; base address of `fs` segment.
#define MSR_FSBASE  0xc0000100
// Address of GSBASE MSR; base address of `gs` segment.
// Swapped with KGSBASE using the `swapgs` instruction.
#define MSR_GSBASE  0xc0000101
// Address of KGSBASE MSR; temporary value for kernel `gs` segment.
// Swapped with GSBASE using the `swapgs` instruction.
#define MSR_KGSBASE 0xc0000102
// Address of EFER MSR; extended feature enable register.
#define MSR_EFER    0xc0000080
// Address of the STAR MSR; CS/SS for user/kernel.
#define MSR_STAR    0xc0000081
// Adress of the LSTAR MSR; entry point for system calls.
#define MSR_LSTAR   0xc0000082
// Address of the FMASK MSR; flags to clear when entering kernel.
#define MSR_FMASK   0xc0000084



#ifndef __ASSEMBLER__
// Read an MSR.
FORCEINLINE static inline uint64_t msr_read(uint32_t address) {
    uint32_t addr = address;
    uint32_t lo;
    uint32_t hi;
    asm("rdmsr" : "=a"(lo), "=d"(hi) : "c"(addr) : "memory");
    return ((uint64_t)hi << 32) | lo;
}

// Write an MSR.
FORCEINLINE static inline void msr_write(uint32_t address, uint64_t value) {
    uint32_t addr = address;
    uint32_t lo   = value;
    uint32_t hi   = value >> 32;
    asm volatile("wrmsr" ::"a"(lo), "d"(hi), "c"(addr) : "memory");
}
#endif
