
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "device/builtin_drivers.h"

#include "kmodule.h"



static void add_builtin_drivers() {
    driver_add(&driver_generic_pcie.base);
    void add_rust_builtin_drivers();
    add_rust_builtin_drivers();
}

static void dummy() {
}

static kmodule_t builtin_drivers = {
    .min_abi = KMODULE_ABI_VER,
    .mod_ver = {1, 0, 0},
    .name    = "drivers",
    .init    = add_builtin_drivers,
    .deinit  = dummy,
};
REGISTER_KMODULE(builtin_drivers);
