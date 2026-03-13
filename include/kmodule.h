
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// Kernel module semver major.
#define KMODULE_ABI_MAJ 1
// Kernel module semver minor.
#define KMODULE_ABI_MIN 0
// Kernel module semver patch.
#define KMODULE_ABI_PAT 0
// Kernel module ABI version.
#define KMODULE_ABI_VER {KMODULE_ABI_MAJ, KMODULE_ABI_MIN, KMODULE_ABI_PAT}



// Kernel module interface.
typedef struct {
    // Minimum kernel module semver (maj, min, pat).
    uint8_t     min_abi[3];
    // Module version.
    uint8_t     mod_ver[3];
    // Module name, [a-zA-Z0-9_].
    char const *name;
    // Module init function.
    void (*init)();
    // Module deinit function.
    void (*deinit)();
} kmodule_t;



// Register a built-in kernel module.
#define REGISTER_KMODULE(kmodule_id)                                                                                   \
    __attribute__((section(".kmodules"))) kmodule_t const *kmodule_id##_kmodule_table_entry = &(kmodule_id);
