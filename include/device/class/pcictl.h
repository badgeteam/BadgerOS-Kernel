
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "device/dev_addr.h"
#include "device/device.h"
#include "device/pci/bar.h"
#include "device/pci/confspace.h"



// Information gleemed from PCI base address register.
typedef struct {
    // BAR space address.
    size_t addr;
    // Length in bytes.
    size_t len;
    // Is I/O; requires write-through policy..
    bool   is_io;
    // Is a 64-bit BAR.
    bool   is_64bit;
    // Is prefetchable; allows caching of reads.
    bool   prefetch;
    // Is valid; if 0, this BAR is not present or valid.
    bool   valid;
} pci_bar_info_t;

// Information about a memory-mapped PCI base address register.
typedef struct {
    pci_bar_info_t info;
    size_t         vaddr;
} pci_bar_handle_t;

// Physical address -> BAR mapping range.
typedef struct {
    // CPU physical address.
    size_t      cpu_paddr;
    // PCI BAR space address.
    pci_paddr_t pci_paddr;
    // Region length in bytes.
    size_t      length;
} pci_bar_range_t;

// Mapping from PCI IRQ to CPU IRQ.
typedef struct {
    // PCI device address.
    dev_pci_addr_t pci_paddr;
    // PCI IRQ pin.
    irqno_t        pci_irqno;
    // Interrupt parent.
    device_t      *irq_parent;
    // CPU IRQ pin.
    irqno_t        parent_irqno;
} pci_irqmap_t;

// PCI controller device; must use CAM (PCI) or ECAM (PCIe).
typedef struct {
    device_t         base;
    // Is a PCIe controller (as opposed to a PCI controller).
    bool             is_pcie;
    // Number of BAR ranges.
    size_t           ranges_len;
    // Physical address -> BAR mapping ranges.
    pci_bar_range_t *ranges;
    // Number of PCI IRQ to CPU IRQ mappings.
    size_t           irqmap_len;
    // PCI IRQ to CPU IRQ mappings.
    pci_irqmap_t    *irqmap;
    // Interrupt mask.
    pci_paddr_t      irqmap_mask;
    // First bus number.
    uint8_t          bus_start;
    // Last bus number.
    uint8_t          bus_end;
} device_pcictl_t;

// PCI controller device driver functions.
typedef struct {
    driver_t base;
    // Read data from the configuration space.
    void (*cam_read)(device_pcictl_t *device, uint32_t addr, uint32_t len, void *data);
    // Write data to the configuration space.
    void (*cam_write)(device_pcictl_t *device, uint32_t addr, uint32_t len, void const *data);
} driver_pcictl_t;



// Enumerate a PCI or PCIe bus, adding or removing devices accordingly.
errno_t device_pcictl_enumerate(device_pcictl_t *device);
// Get a PCI function's BAR information.
void    device_pcictl_bar_info(device_pcictl_t *device, dev_pci_addr_t addr, pci_bar_info_t out_bar_info[6]);
// Map a PCI function's BAR.
// Returns a pointer to the mapped virtual address or NULL if out of virtual memory.
void   *device_pcictl_bar_map(device_pcictl_t *device, pci_bar_info_t bar_info);
// Read data from the configuration space for a specific device.
void    device_pcictl_dev_cam_read(
       device_pcictl_t *device, dev_pci_addr_t dev_addr, uint32_t offset, uint32_t len, void *data
   );
// Write data to the configuration space for a specific device.
void device_pcictl_dev_cam_write(
    device_pcictl_t *device, dev_pci_addr_t dev_addr, uint32_t offset, uint32_t len, void const *data
);
// Read data from the configuration space.
void device_pcictl_cam_read(device_pcictl_t *device, uint32_t addr, uint32_t len, void *data);
// Write data to the configuration space.
void device_pcictl_cam_write(device_pcictl_t *device, uint32_t addr, uint32_t len, void const *data);
// Enable a PCI device's interrupt.
// 0: MSI/MSI-X, 1-4: INTA-INTD.
void device_pcictl_dev_enable_irq(device_pcictl_t *device, dev_pci_addr_t addr, irqno_t irqno, bool enable);
