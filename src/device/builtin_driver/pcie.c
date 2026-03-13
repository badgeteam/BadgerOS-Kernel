
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "badge_strings.h"
#include "device/class/pcictl.h"
#include "device/device.h"
#include "log.h"
#include "malloc.h"



#if CONFIG_ENABLE_DTB
// Extract ranges from DTB.
static errno_t pci_dtb_ranges(device_pcictl_t *device, dtb_handle_t *handle, dtb_node_t *node) {
    dtb_prop_t *ranges = dtb_get_prop(handle, node, "ranges");
    if (!ranges) {
        logk(LOG_ERROR, "Missing ranges for PCI");
        return -EINVAL;
    }

    // PCIe cell counts must match these numbers.
    if (dtb_read_uint(handle, node, "#size-cells") != 2) {
        logk(LOG_ERROR, "Incorrect #size-cells for PCI");
        return -EINVAL;
    }
    if (dtb_read_uint(handle, node, "#address-cells") != 3) {
        logk(LOG_ERROR, "Incorrect #address-cells for PCI");
        return -EINVAL;
    }

    // If ranges is empty, it is identity-mapped.
    if (ranges->content_len == 0) {
        device->ranges_len = 1;
        device->ranges     = malloc(sizeof(pci_bar_range_t));
        if (!device->ranges) {
            return -ENOMEM;
        }
        device->ranges[0] = (pci_bar_range_t){
            .cpu_paddr = 0,
            .pci_paddr = {0},
            .length    = SIZE_MAX,
        };
        return 0;
    }

    // A PCI range mapping is always 7 cells total.
    device->ranges_len = ranges->content_len / (4 * 7);
    device->ranges     = malloc(device->ranges_len * sizeof(pci_bar_range_t));
    if (!device->ranges) {
        return -ENOMEM;
    }
    for (size_t i = 0; i < device->ranges_len; i++) {
        device->ranges[i] = (pci_bar_range_t){
            .cpu_paddr          = dtb_prop_read_cells(handle, ranges, i * 7 + 3, 2),
            .pci_paddr.attr.val = dtb_prop_read_cells(handle, ranges, i * 7 + 0, 1),
            .pci_paddr.addr_hi  = dtb_prop_read_cells(handle, ranges, i * 7 + 1, 1),
            .pci_paddr.addr_lo  = dtb_prop_read_cells(handle, ranges, i * 7 + 2, 1),
            .length             = dtb_prop_read_cells(handle, ranges, i * 7 + 6, 1),
        };
    }

    return 0;
}

// Extract interrupt mappings from DTB.
static errno_t pci_dtb_irqmap(device_pcictl_t *device, dtb_handle_t *handle, dtb_node_t *node) {
    // PCI #interrupt-cells must be 1.
    if (dtb_read_uint(handle, node, "#interrupt-cells") != 1) {
        logk(LOG_ERROR, "Incorrect #interrupt-cells for PCI");
        return -EINVAL;
    }

    dtb_prop_t *interrupt_mask = dtb_get_prop(handle, node, "interrupt-map-mask");
    if (!interrupt_mask) {
        logk(LOG_WARN, "Missing interrupt-map-mask for PCI");
        return -EINVAL;

    } else if (interrupt_mask->content_len != 16) {
        logk(LOG_ERROR, "Incorrect interrupt-map-mask for PCI");
        return -EINVAL;

    } else {
        device->irqmap_mask.attr.val = dtb_prop_read_cell(handle, interrupt_mask, 0);
        device->irqmap_mask.addr_hi  = dtb_prop_read_cell(handle, interrupt_mask, 1);
        device->irqmap_mask.addr_lo  = dtb_prop_read_cell(handle, interrupt_mask, 2);
    }

    dtb_prop_t *interrupt_map = dtb_get_prop(handle, node, "interrupt-map");
    if (!interrupt_map) {
        logk(LOG_ERROR, "Missing interrupt-map for PCI");
        return -EINVAL;
    }

    device->irqmap_len = interrupt_map->content_len / 4 / 6;
    for (size_t i = 0; i < device->irqmap_len; i++) {
        uint32_t  parent_phandle = dtb_prop_read_cell(handle, interrupt_map, i * 6 + 4);
        device_t *parent_device  = device_by_phandle(parent_phandle);
        if (!parent_device) {
            logkf(LOG_ERROR, "Invalid interrupt-map IRQ parent phandle 0x%{u32;x}", parent_phandle);
            return -EINVAL;
        }
        device_pop_ref(parent_device);
    }

    device->irqmap = malloc(sizeof(pci_irqmap_t) * device->irqmap_len);
    if (!device->irqmap) {
        return -ENOMEM;
    }
    for (size_t i = 0; i < device->irqmap_len; i++) {
        device->irqmap[i].pci_paddr.val = dtb_prop_read_cell(handle, interrupt_map, i * 6 + 0);
        // Address stored in cells 1 and 2 is ignored; it is not relevant to interrupt mapping.
        device->irqmap[i].pci_irqno     = dtb_prop_read_cell(handle, interrupt_map, i * 6 + 3);
        uint32_t parent_phandle         = dtb_prop_read_cell(handle, interrupt_map, i * 6 + 4);
        device->irqmap[i].irq_parent    = device_by_phandle(parent_phandle);
        device->irqmap[i].parent_irqno  = dtb_prop_read_cell(handle, interrupt_map, i * 6 + 5);
    }

    return 0;
}
#endif



bool pcie_generic_match(device_info_t *info) {
#if CONFIG_ENABLE_DTB
    if (device_test_dtb_compat(info, 1, (char const *const[]){"pci-host-ecam-generic"})) {
        return true;
    }
#endif
    return false;
}

#if CONFIG_ENABLE_DTB
errno_t pcie_generic_add_ecam_from_dtb(device_pcictl_t *device) {
    // Read bus range.
    dtb_prop_t *bus_range = dtb_get_prop(device->base.info.dtb_handle, device->base.info.dtb_node, "bus-range");
    if (bus_range->content_len != 8) {
        logk(LOG_ERROR, "Incorrect bus-range for PCI");
        return -EINVAL;
    }
    device->bus_start = dtb_prop_read_cell(device->base.info.dtb_handle, bus_range, 0);
    device->bus_end   = dtb_prop_read_cell(device->base.info.dtb_handle, bus_range, 1);

    errno_t res = pci_dtb_ranges(device, device->base.info.dtb_handle, device->base.info.dtb_node);
    if (res < 0) {
        return res;
    }
    res = pci_dtb_irqmap(device, device->base.info.dtb_handle, device->base.info.dtb_node);
    if (res < 0) {
        free(device->ranges);
        device->ranges_len = 0;
        device->ranges     = NULL;
        return res;
    }

    return 0;
}
#endif

errno_t pcie_generic_add(device_t *device) {
#if CONFIG_ENABLE_DTB
    return pcie_generic_add_ecam_from_dtb((device_pcictl_t *)device);
#endif
    return -ENOTSUP;
}

void pcie_generic_remove(device_t *device) {
}

bool pcie_generic_interrupt(device_t *device, irqno_t pin) {
    return false;
}

errno_t pcie_generic_enable_irq(device_t *device, irqno_t pin, bool enable) {
    return -ENOTSUP;
}

void pcie_generic_cam_read(device_pcictl_t *device, uint32_t addr, uint32_t len, void *data) {
    mem_copy(data, (void const *)device->base.info.addrs[0].mmio.vaddr + addr, len);
}

void pcie_generic_cam_write(device_pcictl_t *device, uint32_t addr, uint32_t len, void const *data) {
    mem_copy((void *)device->base.info.addrs[0].mmio.vaddr + addr, data, len);
}



// Built-in generic ECAM PCIe driver.
driver_pcictl_t const driver_generic_pcie = {
    .base.dev_class      = DEV_CLASS_PCICTL,
    .base.match          = pcie_generic_match,
    .base.add            = pcie_generic_add,
    .base.remove         = pcie_generic_remove,
    .base.interrupt      = pcie_generic_interrupt,
    .base.enable_irq_out = pcie_generic_enable_irq,
    .cam_read            = pcie_generic_cam_read,
    .cam_write           = pcie_generic_cam_write,
};
