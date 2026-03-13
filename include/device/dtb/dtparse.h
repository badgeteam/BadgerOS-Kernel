
// SPDX-License-Identifier: MIT

#pragma once

#include "device/dtb/dtb.h"



// Parse the DTB and add found devices.
void dtparse(void *dtb_ptr);

// Dump the DTB.
void dtdump(void *dtb_ptr);
