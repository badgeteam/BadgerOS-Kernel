
// SPDX-License-Identifier: MIT

#pragma once

#ifndef __ASSEMBLER__
#include <stddef.h>
#include <stdint.h>
#endif



/* ==== CPU INFO ==== */

// Kernel runs in M-mode instead of S-mode.
#define RISCV_M_MODE_KERNEL    0
// Number of PMP regions supported by the CPU.
#define RISCV_PMP_REGION_COUNT 16

// Number of interrupt channels (excluding trap handler) in the vector table.
#define RISCV_VT_INT_COUNT   31
// Number of padding words in the vector table.
#define RISCV_VT_PADDING     0
// Bitmask for interrupt cause.
#define RISCV_VT_ICAUSE_MASK 31
// Bitmask for trap cause.
#define RISCV_VT_TCAUSE_MASK 31
