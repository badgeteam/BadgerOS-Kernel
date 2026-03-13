
// SPDX-License-Identifier: MIT

#pragma once

#include "attributes.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// BAR flag: Is I/O BAR.
#define BAR_FLAG_IO         1
// BAR flag: Is a 64-bit memory BAR.
#define BAR_FLAG_64BIT      4
// I/O BAR address mask.
#define BAR_IO_ADDR_MASK    0xfffffffc
// 32-bit memory BAR address mask.
#define BAR_MEM32_ADDR_MASK 0xfffffff0
// 64-bit memory BAR address mask.
#define BAR_MEM64_ADDR_MASK 0xfffffffffffffff0
// Memory BAR flag: Is prefetchable.
#define BAR_FLAG_PREFETCH   8

// PCI base address register.
typedef uint32_t pci_bar_t;
// 64-bit PCI base address register.
typedef uint64_t ALIGNED_TO(4) pci_bar64_t;



// PCI address space types; the `type` field in `pci_bar_pattr_t`.
typedef enum {
    // PCIe ECAM configuration space.
    PCI_ASPACE_ECAM  = 0,
    // I/O space.
    PCI_ASPACE_IO    = 1,
    // 32-bit memory space.
    PCI_ASPACE_MEM32 = 2,
    // 64-bit memory space.
    PCI_ASPACE_MEM64 = 3,
} pci_aspace_t;

// Attributes for PCI BAR physical address.
typedef union {
    struct {
        // Register number; zero for #ranges.
        uint32_t regno    : 8;
        // Function number; zero for #ranges.
        uint32_t func     : 3;
        // Device number; zero for #ranges.
        uint32_t dev      : 5;
        // Bus number; zero for #ranges.
        uint32_t bus      : 8;
        // Address space type.
        uint32_t type     : 2;
        // Reserved.
        uint32_t          : 3;
        // Aliased / small.
        uint32_t t        : 1;
        // Prefetchable.
        uint32_t prefetch : 1;
        // Not relocatable.
        uint32_t fixed    : 1;
    };
    uint32_t val;
} pci_bar_pattr_t;

// PCI BAR physical address.
typedef struct ALIGNED_TO(4) {
    pci_bar_pattr_t attr;
    uint32_t        addr_hi;
    uint32_t        addr_lo;
} pci_paddr_t;
