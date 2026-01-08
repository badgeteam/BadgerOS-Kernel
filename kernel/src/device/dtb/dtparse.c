
// SPDX-License-Identifier: MIT

#include "device/dtb/dtparse.h"

#include "arrays.h"
#include "assertions.h"
#include "badge_strings.h"
#include "device/dev_addr.h"
#include "device/dev_class.h"
#include "device/device.h"
#include "device/dtb/dtb.h"
#include "log.h"
#include "rawprint.h"
#include "sched/cpulocal.h"
#include "set.h"
#include "smp.h"

#if __BYTE_ORDER == __LITTLE_ENDIAN
#define htobe32(x) __builtin_bswap32(x)
#define be32toh(x) __builtin_bswap32(x)
#else
#define htobe32(x) (x)
#define be32toh(x) (x)
#endif



// Determine what type of address is expected.
static bool determine_atype(device_t *parent_device, dev_atype_t *atype) {
    *atype = DEV_ATYPE_MMIO;
    while (parent_device) {
        switch (parent_device->dev_class) {
            case DEV_CLASS_UNKNOWN: // Unknown device, unknown address.
            case DEV_CLASS_BLOCK:   // Has no standard child address format.
            case DEV_CLASS_IRQCTL:  // Has no standard child address format.
            case DEV_CLASS_TTY:     // Has no standard child address format.
            case DEV_CLASS_CHAR:    // Has no standard child address format.
            case DEV_CLASS_AHCI:    // Should not be in the DTB.
                return false;

            case DEV_CLASS_PCICTL: *atype = DEV_ATYPE_PCI; return true;
            case DEV_CLASS_I2CCTL: *atype = DEV_ATYPE_I2C; return true;
        }
    }
    return atype;
}

// Try to extract the address from a DTB node.
static void get_node_addr(
    dtb_handle_t *handle, dtb_node_t *node, uint32_t alen, uint32_t slen, device_t *parent_device, device_info_t *info
) {
    dev_atype_t atype;
    if (!determine_atype(parent_device, &atype)) {
        return;
    }

    dtb_prop_t *reg = dtb_get_prop(handle, node, "reg");
    if (!reg) {
        return;
    }

    size_t n_ents = reg->content_len / 4 / (alen + slen);
    if (reg->content_len != 4 * (alen + slen)) {
        return;
    }

    switch (atype) {
        default: __builtin_unreachable();
        case DEV_ATYPE_AHCI: // Cannot be parsed from DTB.
        case DEV_ATYPE_PCI:  // Cannot be parsed from DTB.
            return;

        case DEV_ATYPE_MMIO:
            for (size_t i = 0; i < n_ents; i++) {
                dev_addr_t ent = {
                    .type       = DEV_ATYPE_MMIO,
                    .mmio.paddr = dtb_prop_read_cells(handle, reg, i * (alen + slen), alen),
                    .mmio.size  = dtb_prop_read_cells(handle, reg, i * (alen + slen) + alen, slen),
                };
                array_len_insert(&info->addrs, sizeof(dev_addr_t), &info->addrs_len, &ent, info->addrs_len);
            }
            return;

        case DEV_ATYPE_I2C:
            if (alen != 1 || slen != 0) {
                return;
            }
            for (size_t i = 0; i < n_ents; i++) {
                dev_addr_t ent = {
                    .type = DEV_ATYPE_I2C,
                    .i2c  = dtb_prop_read_cells(handle, reg, i, 1),
                };
                array_len_insert(&info->addrs, sizeof(dev_addr_t), &info->addrs_len, &ent, info->addrs_len);
            }
            return;
    }
}

// Recursive DTB parsing imlpementation that walks the SOC for devices and registers them all.
static device_t *
    dtparse_impl(dtb_handle_t *handle, dtb_node_t *node, uint32_t alen, uint32_t slen, device_t *parent_device) {
    device_t *device = NULL;

    dtb_prop_t *compat = dtb_get_prop(handle, node, "compatible");
    if (compat) {
        // If a compatible node is present, register it as a device.
        if (parent_device) {
            device_push_ref(parent_device);
        }
        device_info_t info = {
            .parent     = parent_device,
            .dtb_handle = handle,
            .dtb_node   = node,
            .phandle    = dtb_read_uint(handle, node, "phandle"),
        };
        get_node_addr(handle, node, alen, slen, parent_device, &info);
        device = device_add(info);
        if (!device) {
            logkf(LOG_ERROR, "Failed to add DTB device %{cs}", node->name);
        } else {
            logkf(LOG_INFO, "Added DTB device %{cs}", node->name);
            size_t offset = 0;
            while (offset < compat->content_len) {
                char const *substr = (char const *)compat->content + offset;
                size_t      len    = cstr_length_upto(substr, compat->content_len - offset);
                logkf(LOG_INFO, " -> %{char;c;arr}", substr, len);
                offset += len + 1;
            }
        }
    }

    // Walk child nodes.
    uint32_t inner_alen = dtb_read_uint(handle, node, "#address-cells");
    uint32_t inner_slen = dtb_read_uint(handle, node, "#size-cells");
    for (size_t i = 0; i < node->nodes_len; i++) {
        dtb_node_t *child = &node->nodes[i];
        device_t   *child_dev =
            dtparse_impl(handle, child, inner_alen ?: alen, inner_slen ?: slen, device ?: parent_device);
        if (child_dev) {
            device_pop_ref(child_dev);
        }
    }

    return device;
}

// Resolves phandle-based references then activates the device.
static void dtparse_phandles(device_t *device) {
    // Parse interrupt-parent.
    dtb_prop_t *irq_parent_prop = dtb_get_prop(device->info.dtb_handle, device->info.dtb_node, "interrupt-parent");
    if (irq_parent_prop) {
        uint32_t phandle        = dtb_prop_read_uint(device->info.dtb_handle, irq_parent_prop);
        device->info.irq_parent = device_by_phandle(phandle);
        if (!device->info.irq_parent) {
            logkf(
                LOG_ERROR,
                "DTB device %{cs} has invalid interrupt parent phandle 0x%{u32;x}",
                device->info.dtb_node->name,
                phandle
            );
        }
    } else if (device->info.parent) {
        device->info.irq_parent = device->info.parent->info.irq_parent;
        device_push_ref(device->info.irq_parent);
    }

    // Parse interrupts or interrupts-extended.
    dtb_prop_t *irq_ext_prop = dtb_get_prop(device->info.dtb_handle, device->info.dtb_node, "interrupts-extended");
    dtb_prop_t *irq_prop     = dtb_get_prop(device->info.dtb_handle, device->info.dtb_node, "interrupts");
    if (irq_ext_prop && irq_prop) {
        logkf(LOG_ERROR, "DTB device %{cs} has both interrupts and interrupts-extended", device->info.dtb_node->name);
    } else if (irq_prop) {
        if (!device->info.irq_parent) {
            logkf(LOG_ERROR, "DTB device %{cs} is missing an interrupt parent", device->info.dtb_node->name);
            goto skipirq;
        }
        device_t *irq_parent = device->info.irq_parent;

        // Get number of cells for this parent.
        dtb_prop_t *irq_cells_prop =
            dtb_get_prop(device->info.dtb_handle, irq_parent->info.dtb_node, "#interrupt-cells");
        if (!irq_cells_prop) {
            device_pop_ref(irq_parent);
            logkf(LOG_ERROR, "Missing #interrupt-cells for interrupt parent %{cs}", irq_parent->info.dtb_node->name);
            goto skipirq;
        }
        uint32_t irq_cells = dtb_prop_read_uint(device->info.dtb_handle, irq_cells_prop);

        uint32_t tot_cells = irq_prop->content_len / 4;
        for (uint32_t irq = 0; irq < tot_cells / irq_cells; irq++) {
            // Read that many cells for the interrupt number.
            int parent_irqno = (int)dtb_prop_read_cells(device->info.dtb_handle, irq_prop, irq * irq_cells, irq_cells);

            // Add the interrupt link.
            if (parent_irqno >= 0) {
                errno_t res = device_link_irq(device, irq, irq_parent, parent_irqno);
                if (res < 0) {
                    logkf(
                        LOG_ERROR,
                        "Failed to link interrupts[%{u32;d}] for device %{cs}: %{cs} (%{cs})",
                        irq,
                        device->info.dtb_node->name,
                        errno_get_name(-res),
                        errno_get_desc(-res)
                    );
                    break;
                }
            }

            device_pop_ref(irq_parent);
        }

    } else if (irq_ext_prop) {
        uint32_t tot_cells   = irq_ext_prop->content_len / 4;
        uint32_t i           = 0;
        irqno_t  child_irqno = 0;
        while (i < tot_cells) {
            // Get interrupt parent.
            uint32_t  phandle    = dtb_prop_read_cell(device->info.dtb_handle, irq_ext_prop, i);
            device_t *irq_parent = device_by_phandle(phandle);
            if (!irq_parent) {
                logkf(
                    LOG_ERROR,
                    "Invalid interrupts-extended phandle 0x%{u32;x} for %{cs}",
                    phandle,
                    device->info.dtb_node->name
                );
                break;
            }

            // Get number of cells for this parent.
            dtb_prop_t *irq_cells_prop =
                dtb_get_prop(device->info.dtb_handle, irq_parent->info.dtb_node, "#interrupt-cells");
            if (!irq_cells_prop) {
                device_pop_ref(irq_parent);
                logkf(
                    LOG_ERROR,
                    "Missing #interrupt-cells for interrupt parent %{cs}",
                    irq_parent->info.dtb_node->name
                );
                break;
            }
            uint32_t irq_cells = dtb_prop_read_uint(device->info.dtb_handle, irq_cells_prop);

            // Read that many cells for the interrupt number.
            int parent_irqno  = dtb_prop_read_cells(device->info.dtb_handle, irq_ext_prop, i + 1, irq_cells);
            i                += irq_cells + 1;

            // Add the interrupt link.
            if (parent_irqno >= 0) {
                errno_t res = device_link_irq(device, child_irqno, irq_parent, parent_irqno);
                if (res < 0) {
                    logkf(
                        LOG_ERROR,
                        "Failed to link interrupts-extended[%{u32;d}] for device %{cs}: %{cs} (%{cs})",
                        child_irqno,
                        device->info.dtb_node->name,
                        errno_get_name(-res),
                        errno_get_desc(-res)
                    );
                    break;
                }
            }
            child_irqno++;

            device_pop_ref(irq_parent);
        }
    }

skipirq:
    device_activate(device);

    // Resolve for child devices.
    set_foreach(device_t, child_dev, &device->children) {
        dtparse_phandles(child_dev);
    }
}

// Parse the DTB and add found devices.
void dtparse(void *dtb_ptr) {
    // Open the DTB for reading.
    dtb_handle_t *handle = dtb_open(dtb_ptr);
    assert_always(handle);
    dtb_node_t *root = dtb_root_node(handle);

    // The SOC node contains devices for which we may have drivers.
    dtb_node_t *cpus     = dtb_get_node(handle, root, "cpus");
    dtb_node_t *soc      = dtb_get_node(handle, root, "soc");
    uint32_t    soc_alen = dtb_read_uint(handle, soc, "#address-cells");
    uint32_t    soc_slen = dtb_read_uint(handle, soc, "#size-cells");

    // Initialise timers.
    time_init_dtb(handle);
    // Initialise SMP.
    smp_init_dtb(cpus);

    // Walk the CPU nodes to find CPU root interrupt controllers.
    for (size_t i = 0; i < cpus->nodes_len; i++) {
        dtb_node_t *cpu = &cpus->nodes[i];
        dtb_prop_t *reg = dtb_get_prop(handle, cpu, "reg");
        if (!reg) {
            continue;
        }
        int smp_idx = smp_get_cpu(dtb_prop_read_uint(handle, reg));
        if (smp_idx == -1) {
            continue;
        }
        dtb_node_t *irqctl_node = dtb_get_node(handle, cpu, "interrupt-controller");
        if (!irqctl_node) {
            logkf(LOG_ERROR, "Missing interrupt controller for CPU%{d}", smp_idx);
            continue;
        }
        device_t *irqctl = dtparse_impl(handle, irqctl_node, 0, 0, NULL);
        if (!irqctl) {
            logkf(LOG_ERROR, "Failed to add interrupt controller for CPU%{d}", smp_idx);
        } else {
            cpulocal_set_irqctl(smp_idx, irqctl);
        }
    }

    // Walk the SOC node to detect devices and install drivers.
    for (size_t i = 0; i < soc->nodes_len; i++) {
        dtb_node_t *node      = &soc->nodes[i];
        device_t   *child_dev = dtparse_impl(handle, node, soc_alen, soc_slen, NULL);
        if (child_dev) {
            device_pop_ref(child_dev);
        }
    }

    set_t all_devs = device_get_all();
    set_foreach(device_t, dev, &all_devs) {
        if (dev->info.dtb_handle && !dev->info.parent) {
            // Do initialization that requires phandles.
            dtparse_phandles(dev);
        }
        // Drop the references from the set because we won't use it from this set again.
        device_pop_ref(dev);
    }
    set_clear(&all_devs);
}



static void pindent(size_t count) {
    while (count--) {
        rawputc(' ');
        rawputc(' ');
    }
}

static bool isbin(uint8_t const *mem, uint32_t len) {
    if (mem[0] == 0 || mem[len - 1] != 0) {
        return true;
    }
    while (len--) {
        if (*mem && (*mem < 0x20 || *mem > 0x7e)) {
            return true;
        }
        mem++;
    }
    return false;
}

static void escprint(char const *str, uint32_t len) {
    while (len--) {
        if (len == 0 && *str == 0) {
            return;
        } else if (*str == 0) {
            rawputc('\\');
            rawputc('0');
        } else {
            rawputc(*str);
        }
        str++;
    }
}

static void hexprint4(uint32_t const *bin, uint32_t len) {
    for (uint32_t i = 0; i < len; i++) {
        rawprint("0x");
        rawprinthex(be32toh(bin[i]), 8);
        if (i < len - 1) {
            rawputc(' ');
        }
    }
}

static void hexprint1(uint8_t const *bin, uint32_t len) {
    for (uint32_t i = 0; i < len; i++) {
        rawprint("0x");
        rawprinthex(bin[i], 2);
        if (i < len - 1) {
            rawputc(' ');
        }
    }
}

static void dtdump_r(dtb_handle_t *handle, dtb_node_t *node) {
    pindent(node->depth);
    rawprint(node->name);
    rawprint(" {\n");

    for (size_t i = 0; i < node->props_len; i++) {
        dtb_prop_t *prop = &node->props[i];
        pindent(node->depth + 1);
        rawprint(prop->name);
        if (prop->content_len) {
            rawprint(" = ");
            if (isbin((uint8_t *)prop->content, prop->content_len)) {
                rawputc('<');
                if (prop->content_len % 4) {
                    hexprint1((uint8_t *)prop->content, prop->content_len);
                } else {
                    hexprint4((uint32_t *)prop->content, prop->content_len / 4);
                }
                rawputc('>');
            } else {
                rawputc('"');
                escprint((char *)prop->content, prop->content_len);
                rawputc('"');
            }
        }
        rawprint(";\n");
    }

    for (size_t i = 0; i < node->nodes_len; i++) {
        dtb_node_t *subnode = &node->nodes[i];
        dtdump_r(handle, subnode);
    }

    pindent(node->depth);
    rawprint("}\n");
}

// Dump the DTB.
void dtdump(void *dtb_ptr) {
    dtb_handle_t *handle = dtb_open(dtb_ptr);
    if (!handle) {
        return;
    }
    dtb_node_t *root = dtb_root_node(handle);
    if (!root) {
        rawprint("Invalid root node\n");
        return;
    }
    dtdump_r(handle, root);
}
