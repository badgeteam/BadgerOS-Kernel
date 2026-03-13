
// SPDX-License-Identifier: MIT

#pragma once



// Early protocol-dependent initialization; before the boot announcement log.
void bootp_early_init();
// Post-heap protocol-dependent initialization; after the boot announcement log.
void bootp_postheap_init();
// Full protocol-dependent initialization; stage that detects devices.
void bootp_full_init();
// Reclaim all reclaimable memory.
void bootp_reclaim_mem();
// Early putchar.
void bootp_early_putc(char c);
