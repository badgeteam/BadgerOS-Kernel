
// SPDX-License-Identifier: MIT

#pragma once

#include <stdint.h>



// CPUID info.
typedef struct {
    uint32_t eax;
    uint32_t ebx;
    uint32_t ecx;
    uint32_t edx;
} cpuid_t;

// Get CPU information.
static inline cpuid_t cpuid(uint32_t index) __attribute__((const));
static inline cpuid_t cpuid(uint32_t index) {
    register uint32_t eax asm("eax") = 0;
    register uint32_t ebx asm("ebx") = 0;
    register uint32_t ecx asm("ecx") = 0;
    register uint32_t edx asm("edx") = 0;
    asm("cpuid" : "+r"(eax), "=r"(ebx), "=r"(ecx), "=r"(edx));
    return (cpuid_t){eax, ebx, ecx, edx};
}
