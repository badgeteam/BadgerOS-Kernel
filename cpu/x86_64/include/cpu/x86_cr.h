
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// CR0 format.
typedef union {
    struct {
        // Protected mode enable.
        size_t pe : 1;
        // Monitor co-processor.
        size_t mp : 1;
        // Emulation. (TODO: Of what?)
        size_t em : 1;
        // Task switched.
        size_t ts : 1;
        // Extension type. (TODO: Of what?)
        size_t et : 1;
        // Numeric error.
        size_t ne : 1;
        // Reserved.
        size_t    : 10;
        // Write protect.
        size_t wp : 1;
        // Alignment mask. (TODO: Of what?)
        size_t am : 1;
        // Reserved.
        size_t    : 10;
        // Not write-through.
        size_t nw : 1;
        // Cache disable.
        size_t cd : 1;
        // Paging.
        size_t pg : 1;
    };
    size_t val;
} x86_cr0_t;

// CR3 format.
typedef union {
    struct {
        // Reserved.
        size_t     : 3;
        // Page-level write through.
        size_t pwt : 1;
        // Page-level cache disable.
        size_t pcd : 1;
        // Reserved.
        size_t     : 7;
    };
    struct {
        // Address space ID.
        size_t pcid     : 12;
        // Page table root PPN.
        size_t root_ppn : 52;
    };
    size_t val;
} x86_cr3_t;

// CR4 format.
typedef union {
    struct {
        // Virtual-8086 Mode Extensions.
        size_t vme        : 1;
        // Protected Mode Virtual Interrupts.
        size_t pvi        : 1;
        // Time Stamp enabled only in ring 0.
        size_t tsd        : 1;
        // Debugging Extensions.
        size_t de         : 1;
        // Page Size Extension.
        size_t pse        : 1;
        // Physical Address Extension.
        size_t pae        : 1;
        // Machine Check Exception.
        size_t mce        : 1;
        // Page Global Enable.
        size_t pge        : 1;
        // Performance Monitoring Counter Enable.
        size_t pce        : 1;
        // OS support for fxsave and fxrstor instructions.
        size_t osfxsr     : 1;
        // OS Support for unmasked simd floating point exceptions.
        size_t osxmmexcpt : 1;
        // User-Mode Instruction Prevention (SGDT, SIDT, SLDT, SMSW, and STR are disabled in user mode).
        size_t umip       : 1;
        // Reserved.
        size_t            : 1;
        // Virtual Machine Extensions Enable.
        size_t vmxe       : 1;
        // Safer Mode Extensions Enable.
        size_t smxe       : 1;
        // Reserved.
        size_t            : 1;
        // Enables the instructions RDFSBASE, RDGSBASE, WRFSBASE, and WRGSBASE.
        size_t fsgsbase   : 1;
        // PCID Enable.
        size_t pcide      : 1;
        // XSAVE And Processor Extended States Enable.
        size_t osxsave    : 1;
        // Reserved.
        size_t            : 1;
        // Supervisor Mode Executions Protection Enable.
        size_t smep       : 1;
        // Supervisor Mode Access Protection Enable.
        size_t smap       : 1;
        // Enable protection keys for user-mode pages.
        size_t pke        : 1;
        // Enable Control-flow Enforcement Technology.
        size_t cet        : 1;
        // Enable protection keys for supervisor-mode pages.
        size_t pks        : 1;
    };
    size_t val;
} x86_cr4_t;