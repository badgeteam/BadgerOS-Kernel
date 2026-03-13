
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "kmodule.h"



static void amd64_kmodule_initfunc() {
}

static kmodule_t const amd64 = {
    .min_abi = KMODULE_ABI_VER,
    .mod_ver = {1, 0, 0},
    .name    = "amd64",
    .init    = amd64_kmodule_initfunc,
};
REGISTER_KMODULE(amd64);
