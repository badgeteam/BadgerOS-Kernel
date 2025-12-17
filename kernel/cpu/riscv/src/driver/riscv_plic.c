
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "cpu/driver/riscv_plic.h"

#include "assertions.h"
#include "badgeros-abi/errno.h"
#include "cpu/riscv.h"
#include "device/class/irqctl.h"
#include "device/dev_addr.h"
#include "device/device.h"
#include "device/dtb/dtb.h"
#include "list.h"
#include "log.h"
#include "map.h"
#include "panic.h"
#include "smp.h"

#include <stddef.h>

#include <malloc.h>

#define REG_READ(addr)       (*(uint32_t const volatile *)(addr))
#define REG_WRITE(addr, val) (*(uint32_t volatile *)(addr) = (val))
#define REG_SET_BIT(addr, bitno)                                                                                       \
    ({                                                                                                                 \
        size_t   atmp = (addr);                                                                                        \
        uint32_t mask = 1ul << (bitno);                                                                                \
        REG_WRITE(atmp, REG_READ(atmp) | mask);                                                                        \
    })
#define REG_CLEAR_BIT(addr, bitno)                                                                                     \
    ({                                                                                                                 \
        size_t   atmp = (addr);                                                                                        \
        uint32_t mask = 1ul << (bitno);                                                                                \
        REG_WRITE(atmp, REG_READ(atmp) & ~mask);                                                                       \
    })

static bool    riscv_plic_match(device_info_t *device);
static errno_t riscv_plic_add(device_t *device);
static void    riscv_plic_remove(device_t *device);
static bool    riscv_plic_interrupt(device_t *device, irqno_t pin);
static errno_t riscv_plic_enable_irq_out(device_t *device, irqno_t pin, bool enable);
static errno_t riscv_plic_enable_irq_in(device_t *device, irqno_t pin, bool enable);



static bool riscv_plic_match(device_info_t *info) {
    return device_test_dtb_compat(info, 1, (char const *const[]){"riscv,plic0", "sifive,plic-1.0.0"});
}

static errno_t riscv_plic_add(device_t *device) {
    size_t vaddr = device->info.addrs[0].mmio.vaddr;

    // PLIC must have one MMIO address.
    if (device->info.addrs_len != 1 || device->info.addrs[0].type != DEV_ATYPE_MMIO) {
        return -EINVAL;
    }

    // Allocate cookie.
    device->cookie = calloc(1, sizeof(device_riscv_plic_t));
    if (!device->cookie) {
        return -ENOMEM;
    }
    device_riscv_plic_t *cookie = device->cookie;

    assert_always(dtb_read_uint(device->info.dtb_handle, device->info.dtb_node, "#interrupt-cells") == 1);
    cookie->ndev = dtb_read_uint(device->info.dtb_handle, device->info.dtb_node, "riscv,ndev");

    // Select appropriate PLIC context to use per CPU.
    cookie->ctx_by_cpu = calloc(smp_count, sizeof(int));
    if (!cookie->ctx_by_cpu) {
        free(cookie);
        return -ENOMEM;
    }
    // Initialize all to -1 (invalid).
    for (int i = 0; i < smp_count; i++) {
        cookie->ctx_by_cpu[i] = -1;
    }

    // For each outgoing interrupt (PLIC context)
    map_foreach(ent, &device->irq_parents) {
        irqno_t ctx_irqno = (irqno_t)(size_t)ent->key;
        dlist_foreach_node(irqconn_t, conn, (dlist_t *)ent->value) {
            if (conn->irqno != RISCV_INT_EXT)
                continue;

            // Get CPU ID from DTB parent node.
            dtb_node_t *cpu_node = conn->device->info.dtb_node ? conn->device->info.dtb_node->parent : NULL;
            if (!cpu_node)
                continue;
            uint32_t cpu_id      = dtb_read_uint(conn->device->info.dtb_handle, cpu_node, "reg");
            int      logical_cpu = smp_get_cpu(cpu_id);
            if (logical_cpu >= 0 && logical_cpu < smp_count) {
                cookie->ctx_by_cpu[logical_cpu] = (int)(size_t)ctx_irqno;
            }
        }
    }

    // Ensure every logical CPU has a valid PLIC context.
    for (int i = 0; i < smp_count; i++) {
        if (cookie->ctx_by_cpu[i] == -1) {
            logkf(LOG_FATAL, "CPU%{d} missing PLIC context", i);
            panic_abort();
        }
    }

    // Disable all interrupts.
    for (int i = 0; i < smp_count; i++) {
        uint32_t ctx_no = cookie->ctx_by_cpu[i];
        REG_WRITE(PLIC_THRESH_OFF(ctx_no) + vaddr, 0);
    }

    map_foreach(ent, &device->irq_children) {
        irqno_t pin = (irqno_t)(size_t)ent->key;
        logkf(LOG_DEBUG, "Setup PLIC IRQ %{d}", pin);
        for (int i = 0; i < smp_count; i++) {
            uint32_t ctx_no = cookie->ctx_by_cpu[i];
            REG_SET_BIT(PLIC_ENABLE_OFF(ctx_no) + pin / 32 * 4 + vaddr, pin % 32);
        }
    }

    return 0;
}

static void riscv_plic_remove(device_t *device) {
}

static bool riscv_plic_interrupt(device_t *device, irqno_t pin) {
    device_riscv_plic_t *plic   = device->cookie;
    size_t               vaddr  = device->info.addrs[0].mmio.vaddr;
    uint32_t             ctx_no = plic->ctx_by_cpu[smp_cur_cpu()];
    uint32_t             irq    = REG_READ(PLIC_CLAIM_OFF(ctx_no) + vaddr);
    if (irq) {
        REG_WRITE(PLIC_CLAIM_OFF(ctx_no) + vaddr, irq);
        // Interrupt claimed by this CPU.
        return device_forward_interrupt(device, irq);
    } else {
        // Interrupt presumed to have been handled by another CPU.
        return true;
    }
}

static errno_t riscv_plic_enable_irq_out(device_t *device, irqno_t pin, bool enable) {
    return -ENOTSUP;
}

static errno_t riscv_plic_enable_irq_in(device_t *device, irqno_t pin, bool enable) {
    size_t               vaddr = device->info.addrs[0].mmio.vaddr;
    device_riscv_plic_t *plic  = device->cookie;
    REG_WRITE(PLIC_PRIO_OFF + (size_t)pin * 4 + vaddr, enable);
    for (int i = 0; i < smp_count; i++) {
        uint32_t ctx_no = plic->ctx_by_cpu[i];
        if (enable) {
            REG_SET_BIT(PLIC_ENABLE_OFF(ctx_no) + pin / 32 * 4 + vaddr, pin % 32);
        } else {
            REG_CLEAR_BIT(PLIC_ENABLE_OFF(ctx_no) + pin / 32 * 4 + vaddr, pin % 32);
        }
    }
    return 0;
}

static errno_t riscv_plic_cascade_enable(device_t *device, irqno_t irq_in_pin) {
    riscv_plic_enable_irq_in(device, irq_in_pin, true);

    // No need to go enabling `sie` bits here; those are done during `irq_init`.

    return 0;
}



driver_irqctl_t const riscv_plic_driver = {
    .base.dev_class          = DEV_CLASS_IRQCTL,
    .base.match              = riscv_plic_match,
    .base.add                = riscv_plic_add,
    .base.remove             = riscv_plic_remove,
    .base.interrupt          = riscv_plic_interrupt,
    .base.enable_irq_out     = riscv_plic_enable_irq_out,
    .base.enable_irq_in      = riscv_plic_enable_irq_in,
    .base.cascase_enable_irq = riscv_plic_cascade_enable,
};
