
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "kmodule.h"

#include "cpu/driver/riscv_intc.h"
#include "cpu/driver/riscv_plic.h"



static void riscv_kmodule_initfunc() {
    driver_add(&riscv_intc_driver.base);
    driver_add(&riscv_plic_driver.base);
}

static kmodule_t const riscv = {
    .min_abi = KMODULE_ABI_VER,
    .mod_ver = {1, 0, 0},
    .name    = "riscv",
    .init    = riscv_kmodule_initfunc,
};
REGISTER_KMODULE(riscv);
