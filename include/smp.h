
// SPDX-License-Identifier: MIT

#pragma once

#include "device/dtb/dtb.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// Number of detected usable CPU cores.
// Never changes after smp_init.
extern int smp_count;

// Initialise the SMP subsystem.
void smp_init_dtb(dtb_node_t *dtb);

// The the SMP CPU index of the calling CPU.
int smp_cur_cpu();
// Get the SMP CPU index from the CPU ID value.
int smp_get_cpu(size_t cpuid);
