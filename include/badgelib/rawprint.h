
// SPDX-License-Identifier: MIT

#pragma once

#include <stddef.h>
#include <stdint.h>

// Simple printer with specified length.
static inline void rawprint_substr(char const *msg, size_t length) {
}
// Simple printer.
static inline void rawprint(char const *msg) {
}
// Simple printer.
static inline void rawputc(char msg) {
}
// Bin 2 hex printer.
static inline void rawprinthex(uint64_t val, int digits) {
}
// Bin 2 dec printer.
static inline void rawprintudec(uint64_t val, int digits) {
}
// Bin 2 dec printer.
static inline void rawprintdec(int64_t val, int digits) {
}
// Current uptime printer for logging.
static inline void rawprintuptime() {
}
