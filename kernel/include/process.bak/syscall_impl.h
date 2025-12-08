
// SPDX-License-Identifier: MIT

#pragma once

#include "syscall.h"

// Sycall: Exit the process; exit code can be read by parent process.
void syscall_self_exit(int code);
