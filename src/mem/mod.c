
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "mem/vmm.h"

// Higher-half direct map size.
size_t vmm_hhdm_size;
// Higher-half direct map virtual address.
size_t vmm_hhdm_vaddr;
// Higher-half direct map address offset (paddr -> vaddr).
size_t vmm_hhdm_offset;
// Kernel base virtual address.
size_t vmm_kernel_vaddr;
// Kernel base physical address.
size_t vmm_kernel_paddr;
