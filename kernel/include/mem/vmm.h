
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "cpu/mmu.h"
#include "errno.h"
#include "mutex.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#if CONFIG_NOMMU
#error "mem/vmm.h" included in a NOMMU kernel
#endif

// Map memory as read-write.
#define VMM_FLAG_RW  (VMM_FLAG_R | VMM_FLAG_W)
// Map memory as read-execute.
#define VMM_FLAG_RX  (VMM_FLAG_R | VMM_FLAG_X)
// Map memory as read-write-execute.
#define VMM_FLAG_RWX (VMM_FLAG_R | VMM_FLAG_W | VMM_FLAG_X)


// Note: These types are copied from the Rust code, do not change them!

typedef size_t vpn_t;
typedef size_t ppn_t;

// A page table.
typedef struct {
    ppn_t   root_ppn;
    mutex_t mtx;
} pagetable_t;

// Allocator for virtual address ranges.
typedef struct {
    vpn_t free_space;
    void *free_list;
} vma_alloc_t;

// Mutex of allocator for virtual address ranges.
typedef struct {
    mutex_t     inner;
    vma_alloc_t data;
} mutex_vma_alloc_t;

// Memory management context.
typedef struct {
    bool              is_kernel;
    pagetable_t       pagetable;
    mutex_vma_alloc_t vma_alloc;
} vmm_ctx_t;

// Describes the result of a virtual to physical address translation.
typedef struct {
    // Virtual address of page start.
    vpn_t    page_vaddr;
    // Physical address of page start.
    ppn_t    page_paddr;
    // Size of the mapping in pages.
    size_t   size;
    // Physical address.
    size_t   paddr;
    // Flags of the mapping.
    uint32_t flags;
    // Whether the mapping exists; if false, only `vpn` and `size` are valid.
    bool     valid;
} virt2phys_t;



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

// Create a new user page table.
errno_t vmm_create_user_ctx(vmm_ctx_t *ctx_out);
// Destroy a user page table.
void    vmm_destroy_user_ctx(vmm_ctx_t ctx);

// Map a range of memory for the kernel at any virtual address.
// Returns the virtual page number where it was mapped.
errno_t     vmm_map_k(vpn_t *virt_base_out, vpn_t virt_len, ppn_t phys_base, uint32_t flags);
// Map a range of memory for a user page table at a specific virtual address.
errno_t     vmm_map_k_at(vpn_t virt_base, vpn_t virt_len, ppn_t phys_base, uint32_t flags);
// Unmap a range of kernel memory.
void        vmm_unmap_k(vpn_t virt_base, vpn_t virt_len);
// Map a range of memory for a user page table at a specific virtual address.
errno_t     vmm_map_u_at(vmm_ctx_t *ctx, vpn_t virt_base, vpn_t virt_len, ppn_t phys_base, uint32_t flags);
// Unmap a range of user memory.
void        vmm_unmap_u(vmm_ctx_t *ctx, vpn_t virt_base, vpn_t virt_len);
// Translate a virtual to a physical address.
virt2phys_t vmm_virt2phys(vmm_ctx_t *ctx, size_t vaddr);
// Switch to a different user virtual memory context.
void        vmm_ctxswitch(vmm_ctx_t *ctx);
// Switch to the kernel virtual memory context.
void        vmm_ctxswitch_k();
