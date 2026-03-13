
// SPDX-License-Identifier: MIT

#pragma once

// Given stack frame pointer, perform backtrace.
void backtrace_from_ptr(void *frame_pointer);
// Perform backtrace as called.
void backtrace();
