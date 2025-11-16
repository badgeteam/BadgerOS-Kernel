
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "device/device.h"

#include "assertions.h"
#include "badge_strings.h"
#include "cpu/interrupt.h"
#include "device/class/block.h"
#include "device/class/union.h"
#include "device/dev_addr.h"
#include "device/dev_class.h"
#include "device/dtb/dtb.h"
#include "errno.h"
#include "filesystem.h"
#include "list.h"
#include "log.h"
#include "map.h"
#include "mem/vmm.h"
#include "mutex.h"
#include "nanoprintf.h"
#include "panic.h"
#include "set.h"
#include "spinlock.h"
#include "time.h"
#include "todo.h"

#include <stdbool.h>
#include <stddef.h>



// Called after a block device is activated.
errno_t device_block_activated(device_block_t *device);
// Called before a block device is removed.
void    device_block_remove(device_block_t *device);



// ID -> device map.
static map_t    devs_by_id      = PTR_MAP_EMPTY;
// Phandle -> device map.
static map_t    devs_by_phandle = PTR_MAP_EMPTY;
// Device ID counter.
static uint32_t id_ctr;
// Devices list mutex.
mutex_t         devs_mtx     = MUTEX_T_INIT_SHARED;
// Drivers set mutex.
mutex_t         drivers_mtx  = MUTEX_T_INIT_SHARED;
// Set of drivers.
set_t           drivers      = PTR_SET_EMPTY;
// Mutex guarding the act of changing the interrupt graph.
mutex_t         irqconn_mtx  = MUTEX_T_INIT_SHARED;
// Spinlock guarding changes to interrupt graph fields.
spinlock_t      irqconn_lock = SPINLOCK_T_INIT_SHARED;



// Test a device info against a set of DTB compatible strings.
bool device_test_dtb_compat(device_info_t const *info, size_t compats_len, char const *const *compats) {
    if (!info->dtb_node) {
        return false;
    }
    dtb_prop_t *prop = dtb_get_prop(info->dtb_handle, info->dtb_node, "compatible");
    if (!prop) {
        return false;
    }
    uint32_t offset = 0;
    while (offset < prop->content_len) {
        uint32_t len = cstr_length_upto(prop->content + offset, prop->content_len - offset);
        for (size_t i = 0; i < compats_len; i++) {
            if (cstr_length(compats[i]) == len && mem_equals(prop->content + offset, compats[i], len)) {
                return true;
            }
        }
        offset += len + 1;
    }
    return false;
}

// Initialize generic information used by devices, with or without drivers.
static bool device_init(device_union_t *device) {
    device->base.children     = PTR_SET_EMPTY;
    device->base.irq_children = PTR_MAP_EMPTY;
    device->base.irq_parents  = PTR_MAP_EMPTY;
    mutex_init(&device->base.driver_mtx, true);

    for (size_t i = 0; i < device->base.info.addrs_len; i++) {
        if (device->base.info.addrs[i].type == DEV_ATYPE_MMIO) {
            dev_mmio_addr_t *addr          = &device->base.info.addrs[i].mmio;
            dev_mmio_addr_t  aligned_addr  = *addr;
            aligned_addr.size             += aligned_addr.vaddr % CONFIG_PAGE_SIZE;
            aligned_addr.paddr            -= aligned_addr.vaddr % CONFIG_PAGE_SIZE;
            aligned_addr.vaddr            -= aligned_addr.vaddr % CONFIG_PAGE_SIZE;
            size_t vpn;
            if (vmm_map_k(
                    &vpn,
                    (addr->size - 1) / CONFIG_PAGE_SIZE + 1,
                    addr->paddr / CONFIG_PAGE_SIZE,
                    VMM_FLAG_RW | VMM_FLAG_IO | VMM_FLAG_A | VMM_FLAG_D | VMM_FLAG_MMIO
                ) < 0) {
                for (i--; i != SIZE_MAX; i--) {
                    if (device->base.info.addrs[i].type == DEV_ATYPE_MMIO) {
                        dev_mmio_addr_t *addr  = &device->base.info.addrs[i].mmio;
                        size_t           vaddr = addr->vaddr;
                        size_t           size  = addr->size;

                        size  += vaddr % CONFIG_PAGE_SIZE;
                        vaddr += vaddr % CONFIG_PAGE_SIZE;
                        if (size % CONFIG_PAGE_SIZE) {
                            size += CONFIG_PAGE_SIZE - size % CONFIG_PAGE_SIZE;
                        }
                        vmm_unmap_k(vaddr / CONFIG_PAGE_SIZE, size / CONFIG_PAGE_SIZE);
                        addr->vaddr = 0;
                    }
                }
                return false;
            }
            addr->vaddr = vpn * CONFIG_PAGE_SIZE + addr->paddr % CONFIG_PAGE_SIZE;
        }
    }

    return true;
}

// Free all generic information used by devices, with or without drivers.
static void device_deinit(device_union_t *device) {
    assert_dev_drop(device->base.children.len == 0);

    for (size_t i = 0; i < device->base.info.addrs_len; i++) {
        if (device->base.info.addrs[i].type == DEV_ATYPE_MMIO) {
            dev_mmio_addr_t addr  = device->base.info.addrs[i].mmio;
            addr.size            += addr.vaddr % CONFIG_PAGE_SIZE;
            addr.paddr           -= addr.vaddr % CONFIG_PAGE_SIZE;
            addr.vaddr           -= addr.vaddr % CONFIG_PAGE_SIZE;
            if (addr.size % CONFIG_PAGE_SIZE) {
                addr.size += CONFIG_PAGE_SIZE - addr.size % CONFIG_PAGE_SIZE;
            }
            vmm_unmap_k(addr.vaddr / CONFIG_PAGE_SIZE, addr.size / CONFIG_PAGE_SIZE);
        }
    }

    free(device->base.info.addrs);
}


// Called after a driver's add function succeeds.
static errno_t driver_add_succeeded(device_union_t *device) {
    switch (device->base.dev_class) {
        case DEV_CLASS_BLOCK: return device_block_activated(&device->block);
        default: return 0;
    }
}


// Register a device to a certain driver.
// Initializes all data used by devices with drivers.
// Returns -errno on error, 0 if not a match, 1 if a match (adding succeeded).
static errno_bool_t device_add_to_driver(device_t *device, driver_t const *driver, bool skip_match) {
    mutex_acquire(&device->driver_mtx, TIMESTAMP_US_MAX);

    if (device->state != DEV_STATE_ACTIVE) {
        // Cannot add a driver because the device is inactive.
        mutex_release(&device->driver_mtx);
        return 1;
    }

    if (device->driver) {
        // Device had already received a driver.
        mutex_release(&device->driver_mtx);
        return 1;
    }

    dev_class_t dev_class   = device->dev_class;
    bool        match_class = (dev_class == DEV_CLASS_UNKNOWN || dev_class == driver->dev_class);
    if (match_class && (skip_match || driver->match(&device->info))) {
        if (dev_class == DEV_CLASS_UNKNOWN) {
            device->dev_class = driver->dev_class;
        }
        errno_t res = driver->add(device);
        if (res >= 0) {
            device->dev_class = driver->dev_class;

            // Take the irq spinlock around the driver set to guard against partial write of the field during an
            // interrupt.
            assert_dev_keep(irq_disable());
            spinlock_take(&irqconn_lock);
            device->driver = driver;
            spinlock_release(&irqconn_lock);
            irq_enable();

            res = driver_add_succeeded((device_union_t *)device);
            if (res < 0) {
                logkf(LOG_ERROR, "Failed to finish post-driver init for device %{d}", device->id);
                driver->remove(device);

                assert_dev_keep(irq_disable());
                spinlock_take(&irqconn_lock);
                device->driver = NULL;
                spinlock_release(&irqconn_lock);
                irq_enable();

                mutex_release(&device->driver_mtx);
                return 1;
            }

            if (device->driver->post_add) {
                device->driver->post_add(device);
            }

            device_t *parent = device->info.parent;
            if (parent) {
                mutex_acquire_shared(&parent->driver_mtx, TIMESTAMP_US_MAX);
                mutex_release(&device->driver_mtx);
                if (parent->driver && parent->driver->child_got_driver) {
                    parent->driver->child_got_driver(parent, device);
                }
                mutex_release_shared(&parent->driver_mtx);
            } else {
                mutex_release(&device->driver_mtx);
            }

            return 2;
        } else {
            logkf(LOG_ERROR, "driver->add failed: %{cs} (%{cs})", errno_get_name(-res), errno_get_desc(-res));
            if (dev_class == DEV_CLASS_UNKNOWN) {
                device->dev_class = DEV_CLASS_UNKNOWN;
            }
        }
    }

    mutex_release(&device->driver_mtx);
    return 0;
}

// Remove a device from its driver.
// Frees all memory used by devices with drivers.
static void device_remove_from_driver(device_t *device) {
    mutex_acquire(&device->driver_mtx, TIMESTAMP_US_MAX);

    if (device->driver) {
        driver_t const *driver = device->driver;

        device_t *parent = device->info.parent;
        if (parent && parent->driver && parent->driver->child_lost_driver) {
            parent->driver->child_lost_driver(parent, device);
        }

        // Take the irq spinlock around the driver set to guard against partial write of the field during an interrupt.
        assert_dev_keep(irq_disable());
        spinlock_take(&irqconn_lock);
        device->driver = NULL;
        spinlock_release(&irqconn_lock);
        irq_enable();

        // Only actually remove from driver afterward to prevent an interrupt from getting to this driver mid-removal.
        driver->remove(device);
    }

    mutex_release(&device->driver_mtx);
}

// Search for a driver for a device and if found, add it to that driver.
static void device_try_find_driver(device_t *device) {
    set_foreach(driver_t const, driver, &drivers) {
        if (device_add_to_driver(device, driver, false) != 0) {
            return;
        }
    }
}

// Remove a device interrupt link; see `device_link_irq`.
static errno_t device_unlink_irq_impl(device_t *child, irqno_t child_pin, device_t *parent, irqno_t parent_pin);

// Register a new device.
// Takes ownership of any memory in `info`, regardless of success.
// Returns a nonzero ID if successful.
device_t *device_add(device_info_t info) {
    device_union_t *device = calloc(1, sizeof(device_union_t));
    if (!device) {
        goto err0;
    }

    mutex_acquire(&devs_mtx, TIMESTAMP_US_MAX);

    // Insert device into the list.
    uint32_t id = ++id_ctr;
    if (!map_set(&devs_by_id, (void *)(size_t)id, device)) {
        goto err1;
    }
    size_t len = devs_by_phandle.len;
    if (info.phandle && !map_set(&devs_by_phandle, (void *)(size_t)info.phandle, device)) {
        goto err2;
    }
    if (info.phandle) {
        assert_always(devs_by_phandle.len == len + 1);
    }

    // Initialize device data.
    device->base.info     = info;
    device->base.id       = id;
    device->base.refcount = 1;
    device->base.state    = DEV_STATE_INACTIVE;
    if (!device_init(device)) {
        goto err3;
    }

    // Add to parent's set of children.
    device_t *parent = device->base.info.parent;
    if (parent) {
        if (!set_add(&parent->children, device)) {
            goto err4;
        }
        if (parent->driver && parent->driver->child_added) {
            errno_t res = parent->driver->child_added(parent, &device->base);
            if (res < 0) {
                logkf(
                    LOG_ERROR,
                    "parent->driver->child_added failed: %{cs} (%{cs})",
                    errno_get_name(-res),
                    errno_get_desc(-res)
                );
                goto err4;
            }
        }
    }

    device->base.refcount = 2;
    mutex_release(&devs_mtx);

    return &device->base;

err4:
    if (info.phandle) {
        map_remove(&devs_by_phandle, (void *)(size_t)info.phandle);
    }
    map_remove(&devs_by_id, (void *)(size_t)id);
    device_pop_ref(&device->base);
    mutex_release(&devs_mtx);
    return NULL;

err3:
    if (info.phandle) {
        map_remove(&devs_by_phandle, (void *)(size_t)info.phandle);
    }
err2:
    map_remove(&devs_by_id, (void *)(size_t)id);
err1:
    free(device);
err0:
    device_pop_ref(info.parent);
    device_pop_ref(info.irq_parent);
    free(info.addrs);
    mutex_release(&devs_mtx);
    return NULL;
}

// Set the driver for a device explicitly.
// Returns EEXIST if it already has one.
errno_t device_set_driver(device_t *device, driver_t const *driver) {
    mutex_acquire_shared(&drivers_mtx, TIMESTAMP_US_MAX);
    assert_always(set_contains(&drivers, driver));

    mutex_acquire(&device->driver_mtx, TIMESTAMP_US_MAX);
    errno_t res = 0;
    if (device->state != DEV_STATE_ACTIVE) {
        logkf(LOG_WARN, "Device %{u32;x} cannot have a driver set because it is not active", device->id);
        res = -ENOENT;
    }
    mutex_release(&device->driver_mtx);

    if (res == 0) {
        res = device_add_to_driver(device, driver, true);
    }

    mutex_release_shared(&drivers_mtx);
    return res;
}

// Activate a device; search for a driver for the device.
// If no driver could be found, the device is now eligible to get one in the future.
// If this function is not called, no driver will ever be added.
void device_activate(device_t *device) {
    mutex_acquire(&device->driver_mtx, TIMESTAMP_US_MAX);
    if (device->state == DEV_STATE_INACTIVE) {
        device->state = DEV_STATE_ACTIVE;
    }
    mutex_release(&device->driver_mtx);
    device_try_find_driver(device);
    device_t *parent = device->info.parent;
    if (parent) {
        if (parent->driver && parent->driver->child_activated) {
            parent->driver->child_activated(parent, device);
        }
    }
}

// Remove a device and its children.
static uint32_t device_remove_impl(uint32_t id) {
    device_t *device = map_get(&devs_by_id, (void *)(size_t)id);

    if (device) {
        // Disconnect from interrupt parents, if any.
        mutex_acquire(&irqconn_mtx, TIMESTAMP_US_MAX);
        map_foreach(ent, &device->irq_parents) {
            dlist_t *list = ent->value;
            dlist_foreach_node(irqconn_t, conn, list) {
                device_unlink_irq_impl(device, (irqno_t)(size_t)ent->value, conn->device, conn->irqno);
            }
        }
        mutex_release(&irqconn_mtx);

        // First remove child devices, if any.
        set_foreach(device_t, child, &device->children) {
            device_remove_impl(child->id);
        }

        if (device->driver) {
            // Children removed, remove the device itself.
            device_remove_from_driver(device);
        }

        // Remove from parent's set of children.
        device_t *parent = device->info.parent;
        if (parent) {
            if (parent->driver && parent->driver->child_removed) {
                parent->driver->child_removed(parent, device);
            }
            set_remove(&parent->children, device);
        }

        // Remove the device from the list.
        map_remove(&devs_by_id, (void *)(size_t)id);
        device_pop_ref((device_t *)device);
    }

    return device != NULL;
}

// Remove a device and its children.
bool device_remove(uint32_t id) {
    // TODO: requires a mutex so that this blocks until this device's `device_activate` finishes.
    TODO();

    // Recursively remove devices.
    mutex_acquire(&devs_mtx, TIMESTAMP_US_MAX);
    bool success = device_remove_impl(id);
    mutex_release(&devs_mtx);

    return success;
}

// Try to get a reference to a device by ID.
// This reference must be cleaned up by `device_pop_ref` and can be cloned by `device_push_ref`.
device_t *device_by_id(uint32_t id) {
    mutex_acquire_shared(&devs_mtx, TIMESTAMP_US_MAX);

    device_t *dev = map_get(&devs_by_id, (void *)(size_t)id);
    if (dev) {
        dev->refcount++;
    }

    mutex_release_shared(&devs_mtx);
    return dev;
}

// Try to get a reference to a device by DTB phandle.
// This reference must be cleaned up by `device_pop_ref` and can be cloned by `device_push_ref`.
device_t *device_by_phandle(uint32_t phandle) {
    mutex_acquire_shared(&devs_mtx, TIMESTAMP_US_MAX);

    device_t *dev = map_get(&devs_by_phandle, (void *)(size_t)phandle);
    if (dev) {
        dev->refcount++;
    }

    mutex_release_shared(&devs_mtx);
    return dev;
}

// Decrease device reference count.
void device_pop_ref(device_t *device_base) {
    if (!device_base) {
        return;
    }

    device_union_t *device = (device_union_t *)device_base;
    if (--device->base.refcount != 0) {
        return;
    }

    if (device->base.info.parent) {
        device_pop_ref(device->base.info.parent);
    }
    if (device->base.info.irq_parent) {
        device_pop_ref(device->base.info.irq_parent);
    }

    switch (device->base.dev_class) {
        case DEV_CLASS_UNKNOWN: /* NOLINT; no action required. */ break;
        case DEV_CLASS_BLOCK: {
            if (!device->block.no_cache) {
                rtree_clear(&device->block.cache);
            }
        } break;
        case DEV_CLASS_CHAR: /* NOLINT; no action required. */ break;
        case DEV_CLASS_IRQCTL: /* NOLINT; no action required. */ break;
        case DEV_CLASS_TTY: /* NOLINT; no action required. */ break;
        case DEV_CLASS_PCICTL: /* NOLINT; no action required. */ break;
        case DEV_CLASS_I2CCTL: /* NOLINT; no action required. */ break;
        case DEV_CLASS_AHCI: /* NOLINT; no action required. */ break;
    }
    device_deinit(device);
    free(device);
}

// Increase device reference count.
void device_push_ref(device_t *device) {
    device->refcount++;
}



// List all devices; returns a `set_t` of `device_t *` shares.
// This reference must be cleaned up by `device_pop_ref` and can be cloned by `device_push_ref`.
set_t device_get_all() {
    set_t set = PTR_SET_EMPTY;
    mutex_acquire_shared(&devs_mtx, TIMESTAMP_US_MAX);

    map_foreach(ent, &devs_by_id) {
        if (set_add(&set, ent->value)) {
            device_push_ref(ent->value);
        }
    }

    mutex_release_shared(&devs_mtx);
    return set;
}

static dev_addr_t mask_addr(dev_addr_t addr, dev_addr_t mask) {
    switch (addr.type) {
        case DEV_ATYPE_MMIO:
            addr.mmio.paddr &= mask.mmio.paddr;
            addr.mmio.vaddr &= mask.mmio.vaddr;
            addr.mmio.size  &= mask.mmio.size;
            break;
        case DEV_ATYPE_PCI:
            addr.pci.bus  &= mask.pci.bus;
            addr.pci.dev  &= mask.pci.dev;
            addr.pci.func &= mask.pci.func;
            break;
        case DEV_ATYPE_I2C: addr.i2c &= mask.i2c; break;
        case DEV_ATYPE_AHCI:
            addr.ahci.pmul_port &= mask.ahci.pmul_port;
            addr.ahci.port      &= mask.ahci.port;
            addr.ahci.pmul      &= mask.ahci.pmul;
            break;
    }
    return addr;
}

static bool match_addr(dev_addr_t a, dev_addr_t b) {
    switch (a.type) {
        case DEV_ATYPE_MMIO:
            return a.mmio.paddr == b.mmio.paddr && a.mmio.vaddr == b.mmio.vaddr && a.mmio.size == b.mmio.size;
        case DEV_ATYPE_PCI: return a.pci.bus == b.pci.bus && a.pci.dev == b.pci.dev && a.pci.func == b.pci.func;
        case DEV_ATYPE_I2C: return a.i2c == b.i2c;
        case DEV_ATYPE_AHCI:
            return a.ahci.pmul_port == b.ahci.pmul_port && a.ahci.port == b.ahci.port && a.ahci.pmul == b.ahci.pmul;
    }
    return false;
}

static bool match_filter(device_union_t const *device, dev_filter_t const *filter) {
    if (filter->match_class && device->base.dev_class != filter->class) {
        return false;
    }

    if (filter->match_addr) {
        bool has_addr = false;
        for (size_t i = 0; i < device->base.info.addrs_len; i++) {
            dev_addr_t addr = device->base.info.addrs[i];
            if (addr.type != filter->addr.type) {
                continue;
            }
            if (filter->use_addr_mask) {
                addr = mask_addr(addr, filter->addr_mask);
            }
            if (match_addr(addr, filter->addr)) {
                has_addr = true;
                break;
            }
        }
        if (!has_addr) {
            return false;
        }
    }

    if (filter->match_driver && device->base.driver != filter->driver) {
        return false;
    }

    if (filter->match_parent && !filter->parent_id && device->base.info.parent) {
        return false;
    }

    return true;
}

// List all devices by class that match the filter; returns a `set_t` of `device_t *` shares.
// This reference must be cleaned up by `device_pop_ref` and can be cloned by `device_push_ref`.
set_t device_get_filtered(dev_filter_t const *filter) {
    set_t set = PTR_SET_EMPTY;
    mutex_acquire_shared(&devs_mtx, TIMESTAMP_US_MAX);

    if (filter->match_parent && filter->parent_id) {
        device_t *parent = device_by_id(filter->parent_id);
        if (parent) {
            set_foreach(device_union_t, child, &parent->children) {
                if (match_filter(child, filter)) {
                    if (set_add(&set, child)) {
                        device_push_ref(&child->base);
                    }
                }
            }
            device_pop_ref(parent);
        }
    } else {
        map_foreach(ent, &devs_by_id) {
            if (match_filter(ent->value, filter)) {
                if (set_add(&set, ent->value)) {
                    device_push_ref(ent->value);
                }
            }
        }
    }

    mutex_release_shared(&devs_mtx);
    return set;
}



// Helper for safe growing of the interrupt designators dictionary.
__attribute__((always_inline)) static inline dlist_t *device_alloc_irqno(map_t *map, irqno_t irqno) {
    dlist_t *list = map_get(map, (void *)(size_t)irqno);
    if (list) {
        return list;
    }

    list = calloc(1, sizeof(dlist_t));
    if (!list) {
        return NULL;
    }

    if (!map_set(map, (void *)(size_t)irqno, list)) {
        free(list);
        return NULL;
    }

    return list;
}

// Add a device interrupt link; child is the device that generates the interrupt, parent the one that receives it.
// Any device interrupt pin can be connected to any number of opposite pins, but the resulting graph must be acyclic.
// If a device has incoming interrupts then it must be an interrupt controller and only such drivers can match.
errno_t device_link_irq(device_t *child, irqno_t child_pin, device_t *parent, irqno_t parent_pin) {
    logkf(LOG_DEBUG, "device_link_irq(%{d}, %{d}, %{d}, %{d})", child->id, child_pin, parent->id, parent_pin);

    // Enfore both devices are in the tree.
    if (child->state == DEV_STATE_REMOVED || parent->state == DEV_STATE_REMOVED)
        return -EINVAL;

    irqconn_t *child_ent  = NULL;
    irqconn_t *parent_ent = NULL;

    dlist_t *child_list  = device_alloc_irqno(&child->irq_parents, child_pin);
    dlist_t *parent_list = device_alloc_irqno(&parent->irq_children, parent_pin);
    child_ent            = calloc(1, sizeof(irqconn_t));
    parent_ent           = calloc(1, sizeof(irqconn_t));
    if (!child_list || !parent_list || !child_ent || !parent_ent)
        goto nomem;

    child_ent->device  = parent;
    child_ent->irqno   = parent_pin;
    parent_ent->device = child;
    parent_ent->irqno  = child_pin;

    dlist_append(child_list, &child_ent->node);
    dlist_append(parent_list, &parent_ent->node);

    return 0;

nomem:
    free(child_ent);
    free(parent_ent);
    return -ENOMEM;
}

// Remove a device interrupt link; see `device_link_irq`.
static errno_t device_unlink_irq_impl(device_t *child, irqno_t child_irqno, device_t *parent, irqno_t parent_irqno) {
    TODO();
}

// Remove a device interrupt link; see `device_link_irq`.
errno_t device_unlink_irq(device_t *child, irqno_t child_irqno, device_t *parent, irqno_t parent_irqno) {
    // Enfore both devices are in the tree.
    if (child->state == DEV_STATE_REMOVED || parent->state == DEV_STATE_REMOVED) {
        return -EINVAL;
    }

    mutex_acquire(&irqconn_mtx, TIMESTAMP_US_MAX);

    errno_t res = device_unlink_irq_impl(child, child_irqno, parent, parent_irqno);

    mutex_release(&irqconn_mtx);

    return res;
}

// Enable an outgoing interrupt.
errno_t device_enable_irq_out(device_t *device, irqno_t irq_out_pin, bool enabled) {
    mutex_acquire_shared(&device->driver_mtx, TIMESTAMP_US_MAX);
    errno_t res;
    if (!device->driver) {
        res = -EFAULT;
    } else {
        res = device->driver->enable_irq_out(device, irq_out_pin, enabled);
    }
    mutex_release_shared(&device->driver_mtx);
    return res;
}

// Cascade-enable an interrupt output's connected parent pins.
// Does not actually enable this interrupt pin on `device`.
errno_t device_cascade_enable_irq_out(device_t *device, irqno_t out_irqno) {
    dlist_t *list = map_get(&device->irq_parents, (void *)(size_t)out_irqno);
    if (!list) {
        return -ENOENT;
    }
    dlist_foreach_node(irqconn_t, conn, list) {
        device_t *parent = conn->device;
        mutex_acquire_shared(&parent->driver_mtx, TIMESTAMP_US_MAX);
        if (!parent->driver || !parent->driver->cascase_enable_irq) {
            mutex_release_shared(&parent->driver_mtx);
            continue;
        }
        parent->driver->cascase_enable_irq(parent, conn->irqno);
        mutex_release_shared(&parent->driver_mtx);
    }
    return 0;
}

// Enable an incoming interrupt.
errno_t device_enable_irq_in(device_t *device, irqno_t irq_in_pin, bool enabled) {
    mutex_acquire_shared(&device->driver_mtx, TIMESTAMP_US_MAX);
    errno_t res;
    if (!device->driver || !device->driver->enable_irq_in) {
        res = -ENOENT;
    } else {
        res = device->driver->enable_irq_in(device, irq_in_pin, enabled);
    }
    mutex_release_shared(&device->driver_mtx);
    return res;
}

// Send an interrupt to all children on a certain pin.
bool device_forward_interrupt(device_t *device, irqno_t in_irqno) {
    assert_dev_drop(!irq_is_enabled());
    dlist_t *list = map_get(&device->irq_children, (void *)(size_t)in_irqno);
    if (!list) {
        return false;
    }
    bool handled = false;
    dlist_foreach_node(irqconn_t, conn, list) {
        if (conn->device->driver) {
            handled |= conn->device->driver->interrupt(conn->device, conn->irqno);
        }
    }
    return handled;
}



// Notify of a device interrupt.
void device_interrupt(device_t *device, irqno_t irq_pin) {
    assert_dev_drop(!irq_is_enabled());
    spinlock_take_shared(&irqconn_lock);
    if (device->driver) {
        device->driver->interrupt(device, irq_pin);
    }
    spinlock_release_shared(&irqconn_lock);
}



// Register a new driver.
errno_t driver_add(driver_t const *driver) {
    mutex_acquire(&drivers_mtx, TIMESTAMP_US_MAX);
    if (!set_add(&drivers, driver)) {
        mutex_release(&drivers_mtx);
        return -ENOMEM;
    }
    mutex_release(&drivers_mtx);

    // Try to match driverless devices against this driver.
    mutex_acquire(&devs_mtx, TIMESTAMP_US_MAX);
    map_foreach(ent, &devs_by_id) {
        if (device_add_to_driver(ent->value, driver, false) != 0) {
            break;
        }
    }
    mutex_release(&devs_mtx);

    return 0;
}

// Remove a driver.
errno_t driver_remove(driver_t const *driver) {
    // TODO: requires a mutex to be thread-safe with things like e.g. scanning partitions just before `device_activate`
    // returns.
    TODO();

    mutex_acquire(&drivers_mtx, TIMESTAMP_US_MAX);
    if (!set_remove(&drivers, driver)) {
        mutex_release(&drivers_mtx);
        return -ENOENT;
    }
    mutex_release(&drivers_mtx);

    // Remove all devices from this driver.
    map_foreach(ent, &devs_by_id) {
        device_t *dev = ent->value;
        if (dev->driver == driver) {
            device_remove_from_driver(dev);

            // Try to find an alternative driver if possible.
            device_try_find_driver(dev);
        }
    }

    return 0;
}



// Helper function to printf into a file.
__attribute__((format(printf, 2, 3))) static errno64_t printf_to_file(file_t fd, char const *fmt, ...) {
    char    buf[64];
    va_list l;
    va_start(l, fmt);
    int len = npf_vsnprintf(buf, sizeof(buf), fmt, l);
    va_end(l);
    return fs_write(fd, buf, len);
}

// Helper function to print a device info's address.
static errno64_t print_addr(file_t fd, dev_addr_t addr) {
    switch (addr.type) {
        case DEV_ATYPE_MMIO:
            return printf_to_file(fd, "addr: mmio: 0x%llx 0x%zx\n", (long long)addr.mmio.paddr, addr.mmio.size);
        case DEV_ATYPE_PCI:
            return printf_to_file(fd, "addr: pci: %02x:%02x.%u\n", addr.pci.bus, addr.pci.dev, addr.pci.func);
        case DEV_ATYPE_I2C: return printf_to_file(fd, "addr: i2c: 0x%x\n", addr.i2c);
        case DEV_ATYPE_AHCI:
            if (addr.ahci.pmul) {
                return printf_to_file(fd, "addr: ahci_pmul: %u %u\n", addr.ahci.port, addr.ahci.pmul_port);
            } else {
                return printf_to_file(fd, "addr: ahci: %u\n", addr.ahci.port);
            }
    }
    return 0;
}

// Helper function to populate devnode dir's `info` file.
static errno_t populate_info_file(file_t devnode_dir, device_t *device) {
    file_t fd = RETURN_ON_ERRNO_FILE(fs_open(devnode_dir, "info", 4, FS_O_WRITE_ONLY | FS_O_CREATE | FS_O_TRUNCATE));

    char const *type;
    switch (device->dev_class) {
        default: logkf(LOG_WARN, "Unknown device type %{d} in populate_info_file", (int)device->dev_class); break;
        case DEV_CLASS_UNKNOWN: type = "unknown"; break;
        case DEV_CLASS_BLOCK: type = "block"; break;
        case DEV_CLASS_IRQCTL: type = "irqctl"; break;
        case DEV_CLASS_TTY: type = "tty"; break;
        case DEV_CLASS_PCICTL: type = "pcictl"; break;
        case DEV_CLASS_I2CCTL: type = "i2cctl"; break;
        case DEV_CLASS_AHCI: type = "ahci"; break;
    }
    RETURN_ON_ERRNO(printf_to_file(fd, "class: %s\n", type), fs_file_drop(fd));

    RETURN_ON_ERRNO(printf_to_file(fd, "id: %u\n", device->id), fs_file_drop(fd));

    if (device->info.parent) {
        RETURN_ON_ERRNO(printf_to_file(fd, "parent: %u\n", device->info.parent->id), fs_file_drop(fd));
    }

    if (device->info.irq_parent) {
        RETURN_ON_ERRNO(printf_to_file(fd, "irq_parent: %u\n", device->info.irq_parent->id), fs_file_drop(fd));
    }

    // Print IDs of child devices.
    dev_filter_t const filter = {
        .parent_id = device->id,
    };
    set_t children = device_get_filtered(&filter);
    bool  err      = false;
    set_foreach(device_t, dev, &children) {
        if (printf_to_file(fd, "child: %u\n", dev->id) < 0) {
            err = true;
        }
        device_pop_ref(dev);
    }
    set_clear(&children);
    if (err) {
        fs_file_drop(fd);
        return -EIO;
    }

    for (size_t i = 0; i < device->info.addrs_len; i++) {
        RETURN_ON_ERRNO(print_addr(fd, device->info.addrs[i]), fs_file_drop(fd));
    }

    fs_file_drop(fd);
    return 0;
}

// Add device nodes to a new devtmpfs.
// Called by the VFS after a devtmpfs filesystem is mounted.
errno_t device_devtmpfs_mounted(file_t devtmpfs_root) {
    RETURN_ON_ERRNO(fs_make_file(devtmpfs_root, "by_id", 5, (make_file_spec_t){.type = NODE_TYPE_DIRECTORY}), {
        logkf(LOG_WARN, "Failed to create devnode by_id: %{cs} (%{cs})", errno_get_name(tmp), errno_get_desc(tmp));
    });

    mutex_acquire_shared(&devs_mtx, TIMESTAMP_US_MAX);
    map_foreach(ent, &devs_by_id) {
        device_t *dev = ent->value;
        char      buf[32];
        int       len = npf_snprintf(buf, sizeof(buf), "by_id/%u", dev->id);
        RETURN_ON_ERRNO(fs_make_file(devtmpfs_root, buf, len, (make_file_spec_t){.type = NODE_TYPE_DIRECTORY}), {
            logkf(
                LOG_WARN,
                "Failed to create devnode %{cs}: %{cs} (%{cs})",
                buf,
                errno_get_name(tmp),
                errno_get_desc(tmp)
            );
            mutex_release_shared(&devs_mtx);
        });

        file_t devnode_dir = RETURN_ON_ERRNO_FILE(fs_open(devtmpfs_root, buf, len, FS_O_DIR_ONLY | FS_O_READ_ONLY), {
            logkf(
                LOG_WARN,
                "Failed to open devnode %{cs}: %{cs} (%{cs})",
                buf,
                errno_get_name(tmp.errno),
                errno_get_desc(tmp.errno)
            );
            mutex_release_shared(&devs_mtx);
        });

        RETURN_ON_ERRNO(populate_info_file(devnode_dir, dev), {
            logkf(
                LOG_WARN,
                "Failed to populate devnode %{cs}/info: %{cs} (%{cs})",
                buf,
                errno_get_name(tmp),
                errno_get_desc(tmp)
            );
            fs_file_drop(devnode_dir);
            mutex_release_shared(&devs_mtx);
        });

        mutex_acquire_shared(&dev->driver_mtx, TIMESTAMP_US_MAX);
        if (dev->driver && dev->driver->create_devnodes) {
            dev->driver->create_devnodes(dev, devtmpfs_root, devnode_dir);
        }
        mutex_release_shared(&dev->driver_mtx);

        fs_file_drop(devnode_dir);
    }
    mutex_release_shared(&devs_mtx);

    return 0;
}
