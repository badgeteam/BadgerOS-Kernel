
// SPDX-License-Identifier: MIT

#pragma once

#include "sched/sync/mutex.h"

#include <stddef.h>



// Number of microseconds before log mutex times out.
#define LOG_MUTEX_TIMEOUT 500000

typedef enum {
    LOG_FATAL,
    LOG_ERROR,
    LOG_WARN,
    LOG_INFO,
    LOG_DEBUG,
} log_level_t;

extern mutex_t log_mtx;

// Print the timestamp and prefix for a log message without locking `log_mtx`.
void logk_prefix(log_level_t level);

// Print an unformatted message.
void logk(log_level_t level, char const *msg);
// Print an unformatted message.
void logk_len(log_level_t level, char const *msg, size_t msg_len);
// Print a formatted message according to format_str.
void logkf(log_level_t level, char const *msg, ...);
// Print a hexdump (usually for debug purposes).
void logk_len_hexdump(log_level_t level, char const *msg, size_t msg_len, void const *data, size_t size);
// Print a hexdump, override the address shown (usually for debug purposes).
void logk_len_hexdump_vaddr(
    log_level_t level, char const *msg, size_t msg_len, void const *data, size_t size, size_t vaddr
);
// Print a hexdump (usually for debug purposes).
void logk_hexdump(log_level_t level, char const *msg, void const *data, size_t size);
// Print a hexdump, override the address shown (usually for debug purposes).
void logk_hexdump_vaddr(log_level_t level, char const *msg, void const *data, size_t size, size_t vaddr);

// Print an unformatted message from an interrupt.
// Only use this function in emergencies.
void logk_from_isr(log_level_t level, char const *msg);
// Print an unformatted message from an interrupt.
// Only use this function in emergencies.
void logk_len_from_isr(log_level_t level, char const *msg, size_t msg_len);
// Print a formatted message according to format_str from an interrupt.
// Only use this function in emergencies.
void logkf_from_isr(log_level_t level, char const *msg, ...);
// Print a hexdump (usually for debug purposes).
void logk_len_hexdump_from_isr(log_level_t level, char const *msg, size_t msg_len, void const *data, size_t size);
// Print a hexdump, override the address shown (usually for debug purposes).
void logk_len_hexdump_vaddr_from_isr(
    log_level_t level, char const *msg, size_t msg_len, void const *data, size_t size, size_t vaddr
);
// Print a hexdump (usually for debug purposes) from an interrupt.
// Only use this function in emergencies.
void logk_hexdump_from_isr(log_level_t level, char const *msg, void const *data, size_t size);
// Print a hexdump, override the address shown (usually for debug purposes) from an interrupt.
// Only use this function in emergencies.
void logk_hexdump_vaddr_from_isr(log_level_t level, char const *msg, void const *data, size_t size, size_t vaddr);
