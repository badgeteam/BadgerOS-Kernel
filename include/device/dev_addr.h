
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// Represents a type of way that a device can be addressed by computer.
typedef enum {
    // Memory-mapped I/O.
    DEV_ATYPE_MMIO,
    // PCI or PCI express addressed.
    DEV_ATYPE_PCI,
    // I2C addressed.
    DEV_ATYPE_I2C,
    // SATA ACHI addressed.
    DEV_ATYPE_AHCI,
} dev_atype_t;


// Represents a memory-mapped I/O address.
typedef struct {
    // Base address in physical memory.
    uint64_t paddr;
    // Base address in virtual memory.
    size_t   vaddr;
    // Byte size in memory.
    size_t   size;
} dev_mmio_addr_t;

// Represents a PCI or PCI express address.
typedef union {
    struct {
        // PCIe cotroller's bus number.
        uint8_t bus  : 8;
        // PCIe bus's device number.
        uint8_t dev  : 5;
        // PCIe device's function number.
        uint8_t func : 3;
    };
    uint16_t val;
} dev_pci_addr_t;

// Represents an AHCI address.
typedef struct {
    // Port number.
    uint8_t port;
    // Port multiplier index.
    uint8_t pmul_port;
    // Is behind a port multiplier.
    bool    pmul;
} dev_ahci_addr_t;


// Represents a device address.
typedef struct {
    dev_atype_t type;
    union {
        dev_mmio_addr_t mmio;
        dev_pci_addr_t  pci;
        uint16_t        i2c;
        dev_ahci_addr_t ahci;
    };
} dev_addr_t;
