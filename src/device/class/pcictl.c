
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "device/class/pcictl.h"

#include "arrays.h"
#include "device/dev_addr.h"
#include "device/device.h"
#include "device/pci/bar.h"
#include "device/pci/confspace.h"
#include "log.h"
#include "mem/vmm.h"
#include "sched/sync/mutex.h"

#include <stddef.h>



// Remove a device with a certain PCIe address.
static void device_pcictl_remove_child(device_pcictl_t *device, uint8_t bus, uint8_t dev) {
}

// Trace a PCI interrupt pin [1,4] to a parent interrupt pin.
// Returns NULL if the interrupt does not exist.
static pci_irqmap_t const *pci_trace_irq_pin(device_pcictl_t *device, dev_pci_addr_t addr, irqno_t pci_irqno) {
    addr.val &= device->irqmap_mask.attr.val;
    for (size_t i = 0; i < device->irqmap_len; i++) {
        if (addr.val == device->irqmap[i].pci_paddr.val && pci_irqno == device->irqmap[i].pci_irqno) {
            return &device->irqmap[i];
        }
    }
    return NULL;
}

// Add a device with a certain PCIe address.
static errno_t device_pcictl_add_child(device_pcictl_t *device, device_info_t info) {
    driver_pcictl_t const *driver = (void *)device->base.driver;
    pcie_hdr_com_t         hdr;

    // Print child device info.
    logkf(LOG_INFO, "Detected PCI device with %{size;d} function%{cs}", info.addrs_len, info.addrs_len == 1 ? "" : "s");
    for (size_t i = 0; i < info.addrs_len; i++) {
        driver->cam_read(
            device,
            (info.addrs[i].pci.bus * 256 + info.addrs[i].pci.dev * 8 + info.addrs[i].pci.func) * 4096,
            sizeof(pcie_hdr_com_t),
            &hdr
        );
        logkf(
            LOG_INFO,
            "  -> %{u8;x}:%{u8;x}.%{u8;d} class %{u8;x}:%{u8;x}:%{u8;x}",
            info.addrs[i].pci.bus,
            info.addrs[i].pci.dev,
            info.addrs[i].pci.func,
            hdr.classcode.baseclass,
            hdr.classcode.subclass,
            hdr.classcode.progif
        );
    }

    // Add the child to the device tree.
    // TODO: This is incorrect; there should be one device node per function.
    device_t *child_dev = device_add(info);
    if (!child_dev) {
        return -ENOMEM;
    }

    // Connect interrupts.
    for (size_t addr = 0; addr < child_dev->info.addrs_len; addr++) {
        for (irqno_t i = 1; i < 5; i++) {
            pci_irqmap_t const *map = pci_trace_irq_pin(device, child_dev->info.addrs[addr].pci, i);
            if (!map) {
                logkf(
                    LOG_ERROR,
                    "Can't find interupt mapping for PCI function %{u8;x}:%{u8;x}.%{u8;x} INT%{c}",
                    child_dev->info.addrs[addr].pci.bus,
                    child_dev->info.addrs[addr].pci.dev,
                    child_dev->info.addrs[addr].pci.func,
                    'A' - 1 + i
                );
                device_remove(child_dev->id);
                device_pop_ref(child_dev);
                return -EINVAL;
            }

            device_link_irq(child_dev, i, map->irq_parent, map->parent_irqno);
        }
    }

    // Activate child.
    device_activate(child_dev);
    device_pop_ref(child_dev);

    return 0;
}

// PCIe ECAM device detection function.
static errno_t ecam_dev_detect(device_pcictl_t *device, uint8_t bus, uint8_t dev) {
    // Read first function info.
    driver_pcictl_t const *driver = (void *)device->base.driver;
    pcie_hdr_com_t         hdr;
    driver->cam_read(device, (bus * 256 + dev * 8) * 4096, sizeof(pcie_hdr_com_t), &hdr);

    if (hdr.vendor_id == 0xffff) {
        // No device at this location.
        device_pcictl_remove_child(device, bus, dev);
        return 0;
    }

    // Construct device info to contain all function numbers.
    device_push_ref(&device->base);
    device_info_t info = {
        .addrs      = NULL,
        .addrs_len  = 0,
        .dtb_handle = NULL,
        .dtb_node   = NULL,
        .parent     = &device->base,
    };

    // Insert function into device addresses.
    dev_addr_t addr = {
        .type     = DEV_ATYPE_PCI,
        .pci.bus  = bus,
        .pci.dev  = dev,
        .pci.func = 0,
    };
    if (!array_len_insert(&info.addrs, sizeof(dev_addr_t), &info.addrs_len, &addr, info.addrs_len)) {
        return -ENOMEM;
    }

    bool multifunc = hdr.hdr_type & 0x80;
    if (multifunc) {
        for (uint8_t func = 1; func < 8; func++) {
            driver->cam_read(device, (bus * 256 + dev * 8 + func) * 4096, sizeof(pcie_hdr_com_t), &hdr);
            if (hdr.vendor_id == 0xffff) {
                // No secondary function at this location.
                continue;
            }

            // Insert function into device addresses.
            addr.pci.func = func;
            if (!array_len_insert(&info.addrs, sizeof(dev_addr_t), &info.addrs_len, &addr, info.addrs_len)) {
                free(info.addrs);
                device_pop_ref(&device->base);
                return -ENOMEM;
            }
        }
    }

    // Register new device.
    return device_pcictl_add_child(device, info);
}

// Enumerate a PCI or PCIe bus, adding or removing devices accordingly.
errno_t device_pcictl_enumerate(device_pcictl_t *device) {
    mutex_lock_shared(&device->base.driver_mtx);
    if (!device->base.driver) {
        mutex_unlock_shared(&device->base.driver_mtx);
        return -EINVAL;
    }
    for (unsigned bus = device->bus_start; bus <= device->bus_end; bus++) {
        for (uint8_t dev = 0; dev < 32; dev++) {
            errno_t res = ecam_dev_detect(device, bus, dev);
            if (res < 0) {
                mutex_unlock_shared(&device->base.driver_mtx);
                return res;
            }
        }
    }
    mutex_unlock_shared(&device->base.driver_mtx);
    return 0;
}

// Get a PCI function's BAR information.
static pci_bar_info_t bar_info_impl(device_pcictl_t *device, uint32_t bar_offset) {
    driver_pcictl_t const *driver = (void *)device->base.driver;
    pci_bar_t              bar32;
    driver->cam_read(device, bar_offset, sizeof(pci_bar_t), &bar32);
    if (bar32 & BAR_FLAG_IO) {
        // I/O BARs.
        pci_bar_t mask = -1;
        driver->cam_write(device, bar_offset, sizeof(pci_bar_t), &mask);
        driver->cam_read(device, bar_offset, sizeof(pci_bar_t), &mask);
        driver->cam_write(device, bar_offset, sizeof(pci_bar_t), &bar32);
        pci_bar_t len = 1 + ~(mask & BAR_IO_ADDR_MASK);
        return (pci_bar_info_t){
            .is_io    = true,
            .is_64bit = false,
            .prefetch = false,
            .addr     = bar32 & BAR_IO_ADDR_MASK,
            .len      = len,
        };

    } else if (bar32 & BAR_FLAG_64BIT) {
        // 64-bit memory BARs.
        pci_bar64_t bar64;
        driver->cam_read(device, bar_offset, sizeof(pci_bar64_t), &bar64);
        pci_bar64_t mask = -1;
        driver->cam_write(device, bar_offset, sizeof(pci_bar64_t), &mask);
        driver->cam_read(device, bar_offset, sizeof(pci_bar64_t), &mask);
        driver->cam_write(device, bar_offset, sizeof(pci_bar64_t), &bar32);
        pci_bar64_t len = 1 + ~(mask & BAR_MEM64_ADDR_MASK);
        return (pci_bar_info_t){
            .is_io    = false,
            .is_64bit = true,
            .prefetch = mask & BAR_FLAG_PREFETCH,
            .addr     = bar64 & BAR_MEM64_ADDR_MASK,
            .len      = len,
        };

    } else {
        // 32-bit memory BARs.
        pci_bar_t mask = -1;
        driver->cam_write(device, bar_offset, sizeof(pci_bar_t), &mask);
        driver->cam_read(device, bar_offset, sizeof(pci_bar_t), &mask);
        driver->cam_write(device, bar_offset, sizeof(pci_bar_t), &bar32);
        pci_bar_t len = 1 + ~(mask & BAR_MEM32_ADDR_MASK);
        return (pci_bar_info_t){
            .is_io    = false,
            .is_64bit = false,
            .prefetch = mask & BAR_FLAG_PREFETCH,
            .addr     = bar32 & BAR_MEM32_ADDR_MASK,
            .len      = len,
        };
    }
}

// Get a PCI function's BAR information.
void device_pcictl_bar_info(device_pcictl_t *device, dev_pci_addr_t addr, pci_bar_info_t bar_info[6]) {
    uint32_t bar_offset =
        (uint32_t)((addr.bus * 256 + addr.dev * 8 + addr.func) * 4096) + offsetof(pcie_hdr_dev_t, bar);
    for (int i = 0; i < 6; i++) {
        bar_info[i].valid = false;
    }
    for (int i = 0; i < 6;) {
        bar_info[i]  = bar_info_impl(device, bar_offset + sizeof(pci_bar_t) * i);
        i           += bar_info[i].is_64bit + 1;
    }
}

// Map a PCI function's BAR.
// Returns a pointer to the mapped virtual address or NULL if out of virtual memory.
void *device_pcictl_bar_map(device_pcictl_t *device, pci_bar_info_t bar_info) {
    // Find a matching PCI BAR range.
    pci_paddr_t ppa;
    uint64_t    pci_paddr;
    size_t      i;
    for (i = 0; i < device->ranges_len; i++) {
        ppa       = device->ranges[i].pci_paddr;
        pci_paddr = ((uint64_t)ppa.addr_hi << 32) | ppa.addr_lo;
        if ((ppa.attr.type == PCI_ASPACE_IO) == bar_info.is_io && pci_paddr <= bar_info.addr &&
            pci_paddr + device->ranges[i].length >= bar_info.addr + bar_info.len) {
            break;
        }
    }
    if (i >= device->ranges_len) {
        // No matches found.
        logk(LOG_ERROR, "Invalid BAR range specified");
        return NULL;
    }

    // Determine CPU physical address.
    size_t cpu_paddr = device->ranges[i].cpu_paddr + bar_info.addr - pci_paddr;

    // Determine appropriate flags.
    int flags = VMM_FLAG_RW | VMM_FLAG_NC;
    if (!ppa.attr.prefetch) {
        flags |= VMM_FLAG_IO;
    }

    // Create MMU mapping.
    size_t vaddr;
    if (vmm_map_k(&vaddr, (bar_info.len - 1) / CONFIG_PAGE_SIZE + 1, cpu_paddr / CONFIG_PAGE_SIZE, flags) < 0) {
        return NULL;
    }

    return (void *)vaddr;
}

// Read data from the configuration space for a specific device.
void device_pcictl_dev_cam_read(
    device_pcictl_t *device, dev_pci_addr_t dev_addr, uint32_t offset, uint32_t len, void *data
) {
    driver_pcictl_t const *driver = (void *)device->base.driver;
    driver->cam_read(device, offset + (dev_addr.bus * 256 + dev_addr.dev * 8 + dev_addr.func) * 4096, len, data);
}

// Write data to the configuration space for a specific device.
void device_pcictl_dev_cam_write(
    device_pcictl_t *device, dev_pci_addr_t dev_addr, uint32_t offset, uint32_t len, void const *data
) {
    driver_pcictl_t const *driver = (void *)device->base.driver;
    driver->cam_write(device, offset + (dev_addr.bus * 256 + dev_addr.dev * 8 + dev_addr.func) * 4096, len, data);
}

// Read data from the configuration space.
void device_pcictl_cam_read(device_pcictl_t *device, uint32_t addr, uint32_t len, void *data) {
    driver_pcictl_t const *driver = (void *)device->base.driver;
    driver->cam_read(device, addr, len, data);
}

// Write data to the configuration space.
void device_pcictl_cam_write(device_pcictl_t *device, uint32_t addr, uint32_t len, void const *data) {
    driver_pcictl_t const *driver = (void *)device->base.driver;
    driver->cam_write(device, addr, len, data);
}
