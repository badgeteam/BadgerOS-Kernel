
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once



// Distinguishes classes of device by purpose.
typedef enum {
    // Unknown class.
    DEV_CLASS_UNKNOWN,
    // Block devices, e.g. SSDs, CD drives, etc.
    DEV_CLASS_BLOCK,
    // Interrupt controllers.
    DEV_CLASS_IRQCTL,
    // TTY devices, e.g. USB CDC-ACM, kernel console, etc.
    DEV_CLASS_TTY,
    // Misc. character devices, e.g. /dev/null.
    DEV_CLASS_CHAR,
    // PCI or PCIe controllers.
    DEV_CLASS_PCICTL,
    // I2C controllers.
    DEV_CLASS_I2CCTL,
    // AHCI controllers.
    DEV_CLASS_AHCI,
} dev_class_t;
