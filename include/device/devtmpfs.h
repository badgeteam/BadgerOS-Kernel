
// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "device/device.h"

// Add device nodes to a new devtmpfs.
// Called by the VFS after a devtmpfs filesystem is mounted.
errno_t device_devtmpfs_mounted(file_t devtmpfs_root);
