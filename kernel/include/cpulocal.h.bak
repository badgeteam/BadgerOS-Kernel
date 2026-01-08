
// SPDX-License-Identifier: MIT

#pragma once

#include "cpu/arch_cpulocal.h"
#include "device/class/irqctl.h"
#include "rcu.h"
#include "time_private.h"

#include <stddef.h>



// CPU-local scheduler data.
typedef struct sched_cpulocal sched_cpulocal_t;

// CPU-local data.
typedef struct cpulocal {
    // Current CPU ID.
    size_t            cpuid;
    // Current SMP CPU inder.
    int               cpu;
    // ISR stack top.
    size_t            isr_stack_top;
    // ISR stack bottom.
    size_t            isr_stack_bottom;
    // CPU-local scheduler data.
    sched_cpulocal_t *sched;
    // CPU-local RCU data.
    rcu_cpulocal_t    rcu;
    // CPU-local timer data.
    time_cpulocal_t   time;
    // Arch-specific CPU-local data.
    arch_cpulocal_t   arch;
    // CPU's root interrupt controller.
    device_t         *root_irqctl;
} cpulocal_t;

// Per-CPU CPU-local data.
extern cpulocal_t *cpulocal;

#include "scheduler/types.h"
