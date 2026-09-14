
// SPDX-License-Identifier: MIT

#pragma once

#include <stddef.h>



typedef enum {
    LOG_FATAL,
    LOG_ERROR,
    LOG_WARN,
    LOG_INFO,
    LOG_DEBUG,
} log_level_t;

// Print the timestamp and prefix for a log message without locking `log_mtx`.
static inline void logk_prefix(log_level_t level) {
}

// Print an unformatted message.
static inline void logk(log_level_t level, char const *msg) {
}
// Print an unformatted message.
static inline void logk_len(log_level_t level, char const *msg, size_t msg_len) {
}
// Print a formatted message according to format_str.
static inline void logkf(log_level_t level, char const *msg, ...) {
}
// Print a hexdump (usually for debug purposes).
static inline void logk_len_hexdump(log_level_t level, char const *msg, size_t msg_len, void const *data, size_t size) {
}
// Print a hexdump, override the address shown (usually for debug purposes).
static inline void logk_len_hexdump_vaddr(
    log_level_t level, char const *msg, size_t msg_len, void const *data, size_t size, size_t vaddr
) {
}
// Print a hexdump (usually for debug purposes).
static inline void logk_hexdump(log_level_t level, char const *msg, void const *data, size_t size) {
}
// Print a hexdump, override the address shown (usually for debug purposes).
static inline void logk_hexdump_vaddr(log_level_t level, char const *msg, void const *data, size_t size, size_t vaddr) {
}

// Print an unformatted message from an interrupt.
// Only use this function in emergencies.
static inline void logk_from_isr(log_level_t level, char const *msg) {
}
// Print an unformatted message from an interrupt.
// Only use this function in emergencies.
static inline void logk_len_from_isr(log_level_t level, char const *msg, size_t msg_len) {
}
// Print a formatted message according to format_str from an interrupt.
// Only use this function in emergencies.
static inline void logkf_from_isr(log_level_t level, char const *msg, ...) {
}
// Print a hexdump (usually for debug purposes).
static inline void
    logk_len_hexdump_from_isr(log_level_t level, char const *msg, size_t msg_len, void const *data, size_t size) {
}
// Print a hexdump, override the address shown (usually for debug purposes).
static inline void logk_len_hexdump_vaddr_from_isr(
    log_level_t level, char const *msg, size_t msg_len, void const *data, size_t size, size_t vaddr
) {
}
// Print a hexdump (usually for debug purposes) from an interrupt.
// Only use this function in emergencies.
static inline void logk_hexdump_from_isr(log_level_t level, char const *msg, void const *data, size_t size) {
}
// Print a hexdump, override the address shown (usually for debug purposes) from an interrupt.
// Only use this function in emergencies.
static inline void
    logk_hexdump_vaddr_from_isr(log_level_t level, char const *msg, void const *data, size_t size, size_t vaddr) {
}
