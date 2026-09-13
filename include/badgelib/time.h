
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define TIMESTAMP_US_MIN INT64_MIN
#define TIMESTAMP_US_MAX INT64_MAX

typedef int64_t timestamp_us_t;

/// Posix nanoseconds timestamp.
typedef struct {
    /// Seconds (excluding leap) since 00:00, Jan 1 1970 UTC.
    long sec;
    /// Nanoseconds after [`Self::sec`].
    long nsec;
} timespec_t;

// Get current time in microseconds.
timestamp_us_t time_us();
