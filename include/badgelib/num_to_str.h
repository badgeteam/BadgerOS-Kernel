
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// Table used to convert to hexadecimal.
// The contents are 0-9 and uppercase A-F.
// To obtain lowercase, bitwise OR with 0x20.
extern char const hexadecimal_table[16];

// Exactly 16-bit binary to decimal into a 5-char buffer.
void   num_uint16_to_str(uint16_t val, char outbuf[5]);
// Exactly 32-bit binary to decimal into a 10-char buffer.
void   num_uint32_to_str(uint32_t val, char outbuf[10]);
// Exactly 64-bit binary to decimal into a 20-char buffer.
void   num_uint64_to_str(uint64_t val, char outbuf[20]);
// Up to 64-bit binary to decimal into a 20-char buffer.
// Pads the buffer with zeroes.
// Returns the amount of non-zero digits, which may be 0.
size_t num_uint_to_str(uint64_t val, char outbuf[20]);

// Generic signed binary to decimal.
// Places a NULL terminator at the end.
// Returns the length of the generated string.
size_t int_to_cstr(int64_t val, char *outbuf, size_t size);
// Generic unsigned binary to decimal.
// Places a NULL terminator at the end.
// Returns the length of the generated string.
size_t uint_to_cstr(uint64_t val, char *outbuf, size_t size);

// Generic signed binary to decimal.
// Places a NULL terminator if the buffer is not filled entirely.
// Returns the length of the generated string.
size_t int_to_cstr_packed(int64_t val, char *outbuf, size_t size);
// Generic unsigned binary to decimal.
// Places a NULL terminator if the buffer is not filled entirely.
// Returns the length of the generated string.
size_t uint_to_cstr_packed(uint64_t val, char *outbuf, size_t size);
