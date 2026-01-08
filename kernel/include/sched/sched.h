// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#pragma once

#include "errno.h"
#include "time.h"



// Get the current thread's control block pointer as a reference.
void const *thread_current();
// Sleep the current thread for a microsecond timestamp.
// May return -EINTR if signalled.
errno_t     thread_sleep(timestamp_us_t timeout);
// Yield the current thread's execution.
// Calling this repeatedly can cause deprioritization.
void        thread_yield();
