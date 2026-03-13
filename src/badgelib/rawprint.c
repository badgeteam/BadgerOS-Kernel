
// SPDX-License-Identifier: MIT

#include "rawprint.h"

#include "bootp.h"
#include "num_to_str.h"
#include "time.h"

#include <stddef.h>

char const hextab[] = "0123456789ABCDEF";

// Simple printer with specified length.
void rawprint_substr(char const *msg, size_t length) {
    while (length--) {
        rawputc(*msg);
        msg++;
    }
}

// Simple printer.
void rawprint(char const *msg) {
    for (; *msg; msg++) rawputc(*msg);
}

// Simple printer.
void rawputc(char msg) {
    static char prev = 0;
    if (msg == '\r') {
        bootp_early_putc('\r');
        bootp_early_putc('\n');
    } else if (msg == '\n') {
        if (prev != '\r') {
            bootp_early_putc('\r');
            bootp_early_putc('\n');
        }
    } else {
        bootp_early_putc(msg);
    }
    prev = msg;
}

// Bin 2 hex printer.
void rawprinthex(uint64_t val, int digits) {
    for (; digits > 0; digits--) {
        rawputc(hextab[(val >> (digits * 4 - 4)) & 15]);
    }
}

// Bin 2 dec printer.
void rawprintudec(uint64_t val, int digits) {
    char   buf[20];
    size_t buf_digits = num_uint_to_str(val, buf);
    if (digits < (int)buf_digits)
        digits = (int)buf_digits;
    else if (digits > (int)sizeof(buf))
        digits = sizeof(buf);
    if (digits < 1)
        digits = 1;
    rawprint_substr(buf + 20 - digits, digits);
}

// Bin 2 dec printer.
void rawprintdec(int64_t val, int digits) {
    if (val < 0) {
        rawputc('-');
        val = -val;
    }
    rawprintudec(val, digits);
}

// Current uptime printer for logging.
void rawprintuptime() {
    char   buf[20];
    size_t digits = num_uint_to_str(time_us() / 1000, buf);
    if (digits < 8)
        digits = 8;

    rawputc('[');
    rawprint_substr(buf + 20 - digits, digits - 3);
    rawputc('.');
    rawprint_substr(buf + 17, 3);
    rawputc(']');
}
