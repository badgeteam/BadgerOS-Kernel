
// SPDX-License-Identifier: MIT

#include "log.h"

#include "badge_format_str.h"
#include "badge_strings.h"
#include "rawprint.h"
#include "sched/sync/mutex.h"

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define LOGK_HEXDUMP_COLS   16
#define LOGK_HEXDUMP_GROUPS 4

#define isvalidlevel(level) ((level) >= 0 && (level) < 5)

mutex_t log_mtx = MUTEX_T_INIT;



static char const *const prefix[] = {
    "FATAL ",
    "ERROR ",
    "WARN  ",
    "INFO  ",
    "DEBUG ",
};

static char const *const colcode[] = {
    "\033[31m",
    "\033[31m",
    "\033[33m",
    "\033[32m",
    "\033[34m",
};

static char const *const term = "\033[0m\n";



// Print the timestamp and prefix for a log message without locking `log_mtx`.
void logk_prefix(log_level_t level) {
    if (isvalidlevel(level))
        rawprint(colcode[level]);
    rawprintuptime();
    rawputc(' ');
    if (isvalidlevel(level))
        rawprint(prefix[level]);
    else
        rawprint("      ");
}

static bool putccb(char const *msg, size_t len, void *cookie) {
    (void)cookie;
    rawprint_substr(msg, len);
    return true;
}



// Print an unformatted message.
void logk(log_level_t level, char const *msg) {
    bool acq = mutex_lock(&log_mtx) >= 0;
    logk_from_isr(level, msg);
    if (acq)
        mutex_unlock(&log_mtx);
}

// Print an unformatted message.
void logk_len(log_level_t level, char const *msg, size_t msg_len) {
    bool acq = mutex_lock(&log_mtx) >= 0;
    logk_len_from_isr(level, msg, msg_len);
    if (acq)
        mutex_unlock(&log_mtx);
}

// Print a formatted message according to format_str.
void logkf(log_level_t level, char const *msg, ...) {
    bool acq = mutex_lock(&log_mtx) >= 0;
    logk_prefix(level);
    va_list vararg;
    va_start(vararg, msg);
    size_t msg_len = cstr_length(msg);
    if (msg_len >= 2 && msg[msg_len - 2] == '\r' && msg[msg_len - 1] == '\n')
        msg_len -= 2;
    if (msg_len && msg[msg_len - 1] == '\n') {
        msg_len--;
    }
    format_str_va(msg, msg_len, putccb, NULL, vararg);
    va_end(vararg);
    rawprint(term);
    if (acq)
        mutex_unlock(&log_mtx);
}

// Print a hexdump (usually for debug purposes).
void logk_len_hexdump(log_level_t level, char const *msg, size_t msg_len, void const *data, size_t size) {
    bool acq = mutex_lock(&log_mtx) >= 0;
    logk_len_hexdump_vaddr_from_isr(level, msg, msg_len, data, size, (size_t)data);
    if (acq)
        mutex_unlock(&log_mtx);
}

// Print a hexdump, override the address shown (usually for debug purposes).
void logk_len_hexdump_vaddr(
    log_level_t level, char const *msg, size_t msg_len, void const *data, size_t size, size_t vaddr
) {
    bool acq = mutex_lock(&log_mtx) >= 0;
    logk_len_hexdump_vaddr_from_isr(level, msg, msg_len, data, size, vaddr);
    if (acq)
        mutex_unlock(&log_mtx);
}

// Print a hexdump (usually for debug purposes).
void logk_hexdump(log_level_t level, char const *msg, void const *data, size_t size) {
    bool acq = mutex_lock(&log_mtx) >= 0;
    logk_len_hexdump_vaddr_from_isr(level, msg, cstr_length(msg), data, size, (size_t)data);
    if (acq)
        mutex_unlock(&log_mtx);
}

// Print a hexdump, override the address shown (usually for debug purposes).
void logk_hexdump_vaddr(log_level_t level, char const *msg, void const *data, size_t size, size_t vaddr) {
    bool acq = mutex_lock(&log_mtx) >= 0;
    logk_len_hexdump_vaddr_from_isr(level, msg, cstr_length(msg), data, size, vaddr);
    if (acq)
        mutex_unlock(&log_mtx);
}



// Print an unformatted message.
void logk_from_isr(log_level_t level, char const *msg) {
    logk_len_from_isr(level, msg, cstr_length(msg));
}

// Print an unformatted message from an interrupt.
// Only use this function in emergencies.
void logk_len_from_isr(log_level_t level, char const *msg, size_t msg_len) {
    logk_prefix(level);
    if (msg_len >= 2 && msg[msg_len - 2] == '\r' && msg[msg_len - 1] == '\n') {
        msg_len -= 2;
    } else if (msg_len && msg[msg_len - 1] == '\n') {
        msg_len--;
    }
    rawprint_substr(msg, msg_len);
    rawprint(term);
}

// Print a formatted message.
void logkf_from_isr(log_level_t level, char const *msg, ...) {
    logk_prefix(level);
    va_list vararg;
    va_start(vararg, msg);
    format_str_va(msg, cstr_length(msg), putccb, NULL, vararg);
    va_end(vararg);
    rawprint(term);
}



// Print a hexdump (usually for debug purposes).
void logk_len_hexdump_from_isr(log_level_t level, char const *msg, size_t msg_len, void const *data, size_t size) {
    logk_len_hexdump_vaddr_from_isr(level, msg, msg_len, data, size, (size_t)data);
}

// Print a hexdump, override the address shown (usually for debug purposes).
void logk_len_hexdump_vaddr_from_isr(
    log_level_t level, char const *msg, size_t msg_len, void const *data, size_t size, size_t vaddr
) {
    logk_prefix(level);
    if (msg_len >= 2 && msg[msg_len - 2] == '\r' && msg[msg_len - 1] == '\n') {
        msg_len -= 2;
    } else if (msg_len && msg[msg_len - 1] == '\n') {
        msg_len--;
    }
    rawprint_substr(msg, msg_len);
    rawputc('\r');

    uint8_t const *ptr = data;
    for (size_t y = 0; y * LOGK_HEXDUMP_COLS < size; y++) {
        rawprinthex((size_t)vaddr + y * LOGK_HEXDUMP_COLS, sizeof(size_t) * 2);
        rawputc(':');
        size_t x;
        for (x = 0; y * LOGK_HEXDUMP_COLS + x < size && x < LOGK_HEXDUMP_COLS; x++) {
            if ((x % LOGK_HEXDUMP_GROUPS) == 0) {
                rawputc(' ');
            }
            rawputc(' ');
            rawprinthex(ptr[y * LOGK_HEXDUMP_COLS + x], 2);
        }
        for (; x < LOGK_HEXDUMP_GROUPS; x++) {
            if ((x % LOGK_HEXDUMP_GROUPS) == 0) {
                rawputc(' ');
            }
            rawputc(' ');
            rawputc(' ');
            rawputc(' ');
        }
        rawputc(' ');
        rawputc(' ');
        for (x = 0; y * LOGK_HEXDUMP_COLS + x < size && x < LOGK_HEXDUMP_COLS; x++) {
            char c = (char)ptr[y * LOGK_HEXDUMP_COLS + x];
            if (c >= 0x20 && c <= 0x7e) {
                rawputc(c);
            } else {
                rawputc('.');
            }
        }
        rawputc('\n');
    }
    rawprint("\033[0m");
}

// Print a hexdump (usually for debug purposes).
void logk_hexdump_from_isr(log_level_t level, char const *msg, void const *data, size_t size) {
    logk_len_hexdump_vaddr_from_isr(level, msg, cstr_length(msg), data, size, (size_t)data);
}

// Print a hexdump, override the address shown (usually for debug purposes).
void logk_hexdump_vaddr_from_isr(log_level_t level, char const *msg, void const *data, size_t size, size_t vaddr) {
    logk_len_hexdump_vaddr_from_isr(level, msg, cstr_length(msg), data, size, vaddr);
}
