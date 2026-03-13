
// SPDX-License-Identifier: MIT

#include "num_to_str.h"

#include "badge_strings.h"

// Table used to convert to hexadecimal.
// The contents are 0-9 and uppercase A-F.
// To obtain lowercase, bitwise OR with 0x20.
char const hexadecimal_table[16] = {'0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F'};

// Exactly 16-bit binary to decimal into a 5-char buffer.
void num_uint16_to_str(uint16_t val, char outbuf[5]) {
    // This is an implementation of the double dabble algorithm.

    // Shift in first 3 bits.
    uint32_t buf   = val >> 13;
    val          <<= 3;

    // Perform double dabble on the remaining 13 bits.
    for (int i = 0; i < 13; i++) {
        uint32_t mask   = ((buf | (buf >> 1)) & (buf >> 2)) | (buf >> 3);
        buf            += (mask & 0x11111) * 3;
        buf           <<= 1;
        buf            |= val >> 15;
        val           <<= 1;
    }

    // Output the ASCII values.
    for (int i = 0; i < 5; i++) {
        outbuf[4 - i] = '0' + ((buf >> (4 * i)) & 15);
    }
}

// Exactly 32-bit binary to decimal into a 10-char buffer.
void num_uint32_to_str(uint32_t val, char outbuf[10]) {
    // This is an implementation of the double dabble algorithm.

    // Shift in first 3 bits.
    uint64_t buf   = val >> 29;
    val          <<= 3;

    // Perform double dabble on the remaining 29 bits.
    for (int i = 0; i < 29; i++) {
        uint64_t mask   = ((buf | (buf >> 1)) & (buf >> 2)) | (buf >> 3);
        buf            += (mask & 0x1111111111) * 3;
        buf           <<= 1;
        buf            |= val >> 31;
        val           <<= 1;
    }

    // Output the ASCII values.
    for (int i = 0; i < 10; i++) {
        outbuf[9 - i] = '0' + ((buf >> (4 * i)) & 15);
    }
}

// Exactly 64-bit binary to decimal into a 20-char buffer.
void num_uint64_to_str(uint64_t val, char outbuf[20]) {
    // This is an implementation of the double dabble algorithm.

    // Shift in first 3 bits.
    uint64_t buf_lo   = val >> 61;
    uint16_t buf_hi   = 0;
    val             <<= 3;

    // Perform double dabble on the remaining 61 bits.
    for (int i = 0; i < 61; i++) {
        // Increment the digits.
        uint64_t mask  = ((buf_lo | (buf_lo >> 1)) & (buf_lo >> 2)) | (buf_lo >> 3);
        buf_lo        += (mask & 0x1111111111111111) * 3;
        mask           = ((buf_hi | (buf_hi >> 1)) & (buf_hi >> 2)) | (buf_hi >> 3);
        buf_hi        += (mask & 0x1111) * 3;

        // Shift the bits.
        buf_hi <<= 1;
        buf_hi  |= buf_lo >> 63;
        buf_lo <<= 1;
        buf_lo  |= val >> 63;
        val    <<= 1;
    }

    // Output the ASCII values.
    for (int i = 0; i < 16; i++) {
        outbuf[19 - i] = '0' + ((buf_lo >> (4 * i)) & 15);
    }
    for (int i = 0; i < 4; i++) {
        outbuf[3 - i] = '0' + ((buf_hi >> (4 * i)) & 15);
    }
}

// Up to 64-bit binary to decimal into a 20-char buffer.
// Pads the buffer with zeroes.
// Returns the amount of non-zero digits, which may be 0.
size_t num_uint_to_str(uint64_t val, char outbuf[20]) {
    size_t buf_digits;
    if (!(val & 0xffffffffffff0000)) {
        // 16-bit.
        num_uint16_to_str(val, outbuf + 15);
        for (int i = 0; i < 15; i++) {
            outbuf[i] = '0';
        }
        buf_digits = 5;
    } else if (!(val & 0xffffffff00000000)) {
        // 32-bit.
        num_uint32_to_str(val, outbuf + 10);
        for (int i = 0; i < 10; i++) {
            outbuf[i] = '0';
        }
        buf_digits = 10;
    } else {
        // 64-bit.
        num_uint64_to_str(val, outbuf);
        buf_digits = 20;
    }

    // Count number of digits.
    while (buf_digits && outbuf[20 - buf_digits] == '0') buf_digits--;
    return buf_digits;
}



// Generic signed binary to decimal.
// Does not pad the buffer and places a NULL terminator.
// Returns the length of the generated string.
size_t int_to_cstr(int64_t val, char *outbuf, size_t size) {
    if (!size || !outbuf)
        return 0;
    if (size == 1) {
        *outbuf = 0;
        return 0;
    }

    // Add the '-'.
    bool sign = val < 0;
    if (sign) {
        val       = -val;
        outbuf[0] = '-';
    }

    // Convert to decimal.
    char   buf[20];
    size_t digits = num_uint_to_str(val, buf);
    if (!digits)
        digits = 1;

    // Copy the buffer.
    size_t copy_size = digits + sign < size - 1 ? digits : size - 1 - sign;
    mem_copy(&outbuf[sign], &buf[20 - copy_size], copy_size);
    outbuf[sign + copy_size] = 0;

    return copy_size + sign;
}

// Generic unsigned binary to decimal.
// Does not pad the buffer and places a NULL terminator.
// Returns the length of the generated string.
size_t uint_to_cstr(uint64_t val, char *outbuf, size_t size) {
    if (!size || !outbuf)
        return 0;
    if (size == 1) {
        *outbuf = 0;
        return 0;
    }

    // Convert to decimal.
    char   buf[20];
    size_t digits = num_uint_to_str(val, buf);
    if (!digits)
        digits = 1;

    // Copy the buffer.
    size_t copy_size = digits < size - 1 ? digits : size - 1;
    mem_copy(outbuf, &buf[20 - copy_size], copy_size);
    outbuf[copy_size] = 0;

    return copy_size;
}



// Generic signed binary to decimal.
// Does not pad the buffer and places a NULL terminator if the buffer is not filled entirely
// Returns the length of the generated string.
size_t int_to_cstr_packed(int64_t val, char *outbuf, size_t size) {
    if (!size || !outbuf)
        return 0;

    // Add the '-'.
    bool sign = val < 0;
    if (sign) {
        val       = -val;
        outbuf[0] = '-';
    }

    // Convert to decimal.
    char   buf[21];
    size_t digits = num_uint_to_str(val, buf);
    if (!digits)
        digits = 1;

    // Copy the buffer.
    size_t copy_size = digits < size - sign ? digits : size - sign;

    mem_copy(&outbuf[sign], &buf[20 - copy_size], copy_size);
    if (sign + copy_size < size)
        outbuf[sign + copy_size] = 0;

    return copy_size + sign;
}

// Generic unsigned binary to decimal.
// Does not pad the buffer and places a NULL terminator if the buffer is not filled entirely
// Returns the length of the generated string.
size_t uint_to_cstr_packed(uint64_t val, char *outbuf, size_t size) {
    if (!size || !outbuf)
        return 0;
    if (size == 1) {
        *outbuf = '0' | (val % 10);
        return 1;
    }

    // Convert to decimal.
    char   buf[20];
    size_t digits = num_uint_to_str(val, buf);
    if (!digits)
        digits = 1;

    // Copy the buffer.
    size_t copy_size = digits < size ? digits : size;
    mem_copy(outbuf, &buf[20 - copy_size], copy_size);
    if (copy_size < size - 1)
        outbuf[copy_size] = 0;

    return copy_size;
}
