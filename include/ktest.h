
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

typedef enum {
    // Run after `bootp_early_init`.
    KTEST_WHEN_EARLY,
    // Run after PMM is set up.
    KTEST_WHEN_PMM,
    // Run after VMM is set up.
    KTEST_WHEN_VMM,
    // Run after `kernel_heap_init`.
    KTEST_WHEN_HEAP,
    // Run when the scheduler has started.
    KTEST_WHEN_SCHED,
    // Run when filesystem is mounted.
    KTEST_WHEN_ROOTFS,
} ktest_when_t;

// Run test cases of a certain level.
void ktests_runlevel(ktest_when_t level);
