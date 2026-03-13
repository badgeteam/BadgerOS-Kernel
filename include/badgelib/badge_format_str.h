
// SPDX-License-Identifier: MIT

#pragma once

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// Callback function used by format_str.
typedef bool (*format_str_cb_t)(char const *msg, size_t len, void *cookie);

// Format a string and output characters via callback.
//
// Format string types:
//   %%                                     - Print a literal % character.
//   %{cs}                                  - Print a C-String; alias for `%{char;c;nul}`.
//   %{ls}                                  - Print a pointer/length string; alias for `%{char;c;array}`.
//   %{<type;><format><;multiple><;joiner>} - General format string; type and multiple are optional.
//
// Format specifiers:
//   d - Print decimal.
//   x - Print unsigned hexadecimal.
//   o - Print unsigned octal.
//   q - Print unsigned octal.
//   c - Print a single character.
//   Hexadecimal is uppercase if the specifier is uppercase and vice versa.
//
// Type speciciers:
//   (omitted)       - signed int
//   ullong / llong  - unsigned / signed long long
//   ulong  / long   - unsigned / signed long
//   uint   / int    - unsigned / signed int
//   ushort / short  - unsigned / signed short int
//   uchar  / char   - unsigned / signed char
//   u64    / i64    - uint64_t / int64_t
//   u32    / i32    - uint32_t / int32_t
//   u16    / i16    - uint16_t / int16_t
//   u8     / i8     - uint8_t  / int8_t
//   size            - size_t
//   ptrdiff         - ptrdiff_t
//
// Multiples specifiers:
//   (omitted)   - Single occurrance; values are passed directly.
//   nul / null  - NULL-terminated array of values; values are passed by pointer to null-terminated array.
//   arr / array - Pointer+length array of values; values are passed by array pointer followed by array length.
//
// Joiner specifiers:
//   Jointer specifiers specify a string to put between array elements.
//   They can contain any character except '}'.
//
// If one of the format specifiers is malformed, the remaining text is output verbatim.
bool format_str_va(char const *msg, size_t length, format_str_cb_t callback, void *cookie, va_list vararg);
