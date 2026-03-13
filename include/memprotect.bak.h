
// SPDX-License-Identifier: MIT

#pragma once

#include "list.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct mpu_ctx mpu_ctx_t;

#define MEMPROTECT_FLAG_R      0x00000001
#define MEMPROTECT_FLAG_W      0x00000002
#define MEMPROTECT_FLAG_RW     0x00000003
#define MEMPROTECT_FLAG_X      0x00000004
#define MEMPROTECT_FLAG_RX     0x00000005
#define MEMPROTECT_FLAG_WX     0x00000006
#define MEMPROTECT_FLAG_RWX    0x00000007
// Global mapping; visible in all address spaces.
#define MEMPROTECT_FLAG_GLOBAL 0x00000010
// Kernel mapping; accessible to kernel mode instead of user mode.
#define MEMPROTECT_FLAG_KERNEL 0x00000020
// I/O mapping; if possible, mark region as I/O.
#define MEMPROTECT_FLAG_IO     0x00000040
// Non-cachable; if possible, mark region as non-cacheable.
#define MEMPROTECT_FLAG_NC     0x00000080



// At least one naturally-aligned page of zeroes.
extern uint8_t const *memprotect_zeroes;
// At least one naturally-aligned page of zeroes.
extern size_t         memprotect_zeroes_len;

#if !CONFIG_NOMMU

// At least one naturally-aligned page of zeroes.
extern size_t memprotect_zeroes_paddr;

// HHDM length in pages.
extern size_t memprotect_hhdm_pages;
// Kernel virtual page number.
extern size_t memprotect_kernel_vpn;
// Kernel physical page number.
extern size_t memprotect_kernel_ppn;
// Kernel length in pages.
extern size_t memprotect_kernel_pages;

struct mpu_ctx {
    // Linked list node.
    dlist_node_t node;
    // Page table root physical page number.
    size_t       root_ppn;
    // Whether there are any dirty mappings for code.
    bool         needs_ifence;
};

// For systems with VMEM: global MMU context.
extern mpu_ctx_t mpu_global_ctx;
// Virtual to physical lookup results.
typedef struct {
    // Memory protection flags.
    uint32_t flags;
    // Physical address.
    size_t   paddr;
    // Page base vaddr.
    size_t   page_vaddr;
    // Page base paddr.
    size_t   page_paddr;
    // Page size.
    size_t   page_size;
} virt2phys_t;
// Lookup virtual address to physical address.
virt2phys_t memprotect_virt2phys(mpu_ctx_t *ctx, size_t vaddr);
// Allocare a kernel virtual address to a certain physical address.
// Does not map anything to this location.
size_t      memprotect_alloc_vaddr(size_t len);
// Free a virtual address range allocated with `memprotect_alloc_vaddr`.
void        memprotect_free_vaddr(size_t vaddr);

#endif

// Initialise memory protection driver.
void memprotect_early_init();
// Initialise memory protection driver.
void memprotect_postheap_init();
// Initialise memory protection driver.
void memprotect_init();
// Create a memory protection context.
void memprotect_create(mpu_ctx_t *ctx);
// Clean up a memory protection context.
void memprotect_destroy(mpu_ctx_t *ctx);
// Add a memory protection region for user memory.
bool memprotect_u(mpu_ctx_t *ctx, size_t vaddr, size_t paddr, size_t length, uint32_t flags);
// Add a memory protection region for kernel memory.
bool memprotect_k(size_t vaddr, size_t paddr, size_t length, uint32_t flags);
// Commit pending memory protections, if any.
void memprotect_commit(mpu_ctx_t *ctx);
// Swap in memory protections for the current thread.
void memprotect_swap_from_isr();
// Swap in memory protections for a given context.
void memprotect_swap(mpu_ctx_t *ctx);
