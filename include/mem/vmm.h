
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "badgeros-abi/errno.h"
#include "cpu/mmu.h"
#include "sched/sync/mutex.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#if CONFIG_NOMMU
#error "mem/vmm.h" included in a NOMMU kernel
#endif


#define VMM_FLAG_R   1
#define VMM_FLAG_W   2
#define VMM_FLAG_X   4
#define VMM_FLAG_NC  8
#define VMM_FLAG_IO  16
#define VMM_FLAG_RW  (VMM_FLAG_R | VMM_FLAG_W)
#define VMM_FLAG_RX  (VMM_FLAG_R | VMM_FLAG_X)
#define VMM_FLAG_RWX (VMM_FLAG_R | VMM_FLAG_W | VMM_FLAG_X)


// Note: These types are copied from the Rust code, do not change them!

typedef size_t vpn_t;
typedef size_t ppn_t;



// Higher-half direct map virtual address.
extern size_t vmm_hhdm_size;
// Higher-half direct map address offset (paddr -> vaddr).
extern size_t vmm_hhdm_vaddr;
// Higher-half direct map size.
extern size_t vmm_hhdm_offset;
// Kernel base virtual address.
extern size_t vmm_kernel_vaddr;
// Kernel base physical address.
extern size_t vmm_kernel_paddr;



// Initialize the virtual memory subsystem.
void vmm_init();

// Map a range of memory for the kernel at any virtual address.
errno_t vmm_map_k(vpn_t *virt_base_out, vpn_t virt_len, ppn_t phys_base, uint32_t flags);
// Map a range of memory for a kernel page table at a specific virtual address.
errno_t vmm_map_k_at(vpn_t virt_base, vpn_t virt_len, ppn_t phys_base, uint32_t flags);
// Unmap a range of kernel memory.
void    vmm_unmap_k(vpn_t virt_base, vpn_t virt_len);
