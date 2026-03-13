
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#include "device/class/ahci.h"
#include "device/class/block.h"
#include "device/class/pcictl.h"



// Built-in generic ECAM PCIe driver.
extern driver_pcictl_t const driver_generic_pcie;
// Built-in generic SATA AHCI controller driver.
extern driver_ahci_t const   driver_generic_ahci;
// Built-in generic SATA block device driver.
extern driver_block_t const  driver_generic_sata;
