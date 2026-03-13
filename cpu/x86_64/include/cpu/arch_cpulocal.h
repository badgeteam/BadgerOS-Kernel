
// SPDX-License-Identifier: MIT

#pragma once

// Arch-specific CPU-local data.
typedef struct {
    // Pointer to this CPU's TSS struct.
    void *tss;
} arch_cpulocal_t;
