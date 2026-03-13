
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>


// ASCII character to lower case.
static inline char ascii_char_to_lower(char c) __attribute__((const));
static inline char ascii_char_to_lower(char c) {
    if (c >= 'A' && c <= 'Z') {
        return c | 0x20;
    } else {
        return c;
    }
}
// ASCII character to upper case.
static inline char ascii_char_to_upper(char c) __attribute__((const));
static inline char ascii_char_to_upper(char c) {
    if (c >= 'a' && c <= 'z') {
        return c & ~0x20;
    } else {
        return c;
    }
}

// Compute the length of a C-string.
static inline size_t cstr_length(char const *string) {
    return __builtin_strlen(string);
}
// Compute the length of a C-string at most `max_length` characters long.
static inline size_t cstr_length_upto(char const *string, size_t max_len) {
#ifdef __clang__
    size_t strnlen(char const *, size_t);
    return strnlen(string, max_len);
#else
    return __builtin_strnlen(string, max_len);
#endif
}
// Find the first occurrance `value` in C-string `string`.
// Returns -1 when not found.
ptrdiff_t cstr_index(char const *string, char value) __attribute__((pure));
// Find the last occurrance `value` in C-string `string`.
// Returns -1 when not found.
ptrdiff_t cstr_last_index(char const *string, char value) __attribute__((pure));
// Find the first occurrance `value` in C-string `string` from and including `first_index`.
// Returns -1 when not found.
ptrdiff_t cstr_index_from(char const *string, char value, size_t first_index) __attribute__((pure));
// Find the last occurrance `value` in C-string `string` up to and excluding `last_index`.
// Returns -1 when not found.
ptrdiff_t cstr_last_index_upto(char const *string, char value, size_t last_index) __attribute__((pure));

// Duplicate a C-string.
char *cstr_duplicate(char const *a);

// Test the equality of two C-strings.
int cstr_compare(char const *a, char const *b) __attribute__((pure));
// Test the of the first `length` characters equality of two C-strings.
int cstr_prefix_compare(char const *a, char const *b, size_t length) __attribute__((pure));
// Test the equality of two C-strings, case-insensitive.
int cstr_compare_case(char const *a, char const *b) __attribute__((pure));
// Test the of the first `length` characters equality of two C-strings, case-insensitive.
int cstr_prefix_compare_case(char const *a, char const *b, size_t length) __attribute__((pure));

// Test the equality of two C-strings.
bool cstr_equals(char const *a, char const *b) __attribute__((pure));
// Test the of the first `length` characters equality of two C-strings.
bool cstr_prefix_equals(char const *a, char const *b, size_t length) __attribute__((pure));
// Test the equality of two C-strings, case-insensitive.
bool cstr_equals_case(char const *a, char const *b) __attribute__((pure));
// Test the of the first `length` characters equality of two C-strings, case-insensitive.
bool cstr_prefix_equals_case(char const *a, char const *b, size_t length) __attribute__((pure));

// Concatenate a NULL-terminated C-string from `src` onto C-string buffer `dest`.
// This may truncate characters, but not the NULL terminator, if `dest` does not fit `src` entirely.
// Returns the length of the new string in `dest`.
size_t cstr_concat(char *dest, size_t size, char const *src);
// Concatenate a NULL-terminated C-string from `src` onto C-string buffer `dest`.
// This may truncate characters, but not the NULL terminator, if `dest` does not fit `src` entirely.
// Returns the length of the new string in `dest`.
size_t cstr_concat_packed(char *dest, size_t size, char const *src);
// Copy a NULL-terminated C-string from `src` into buffer `dest`.
// WARNING: This may leave strings without NULL terminators if `dest` does not fit `src` entirely.
// Returns the length of the new string in `dest`.
size_t cstr_copy(char *dest, size_t size, char const *src);
// Copy at most `length` bytes of C-string `src` into buffer `dest`.
// WARNING: This may leave strings without NULL terminators if `dest` does not fit `src` entirely.
// Returns the length of the new string in `dest`.
size_t cstr_copy_packed(char *dest, size_t size, char const *src);

// Find the first occurrance of byte `value` in memory `memory`.
// Returns -1 when not found.
ptrdiff_t mem_index(void const *memory, size_t size, uint8_t value) __attribute__((pure));
// Find the first occurrance of byte `value` in memory `memory`.
// Returns -1 when not found.
ptrdiff_t mem_last_index(void const *memory, size_t size, uint8_t value) __attribute__((pure));
// Test the equality of two memory areas.
bool      mem_equals(void const *a, void const *b, size_t size) __attribute__((pure));

// Copy the contents of memory area `src` to memory area `dest`.
// Correct copying is guaranteed even if `src` and `dest` are overlapping regions.
static inline void mem_copy(void *dest, void const *src, size_t size) {
    __builtin_memmove(dest, src, size);
}
// Swap the contents of memory areas `a` and `b`.
// For correct copying, `a` and `b` must not overlap.
void               mem_swap(void *a, void *b, size_t size);
// Set the contents of memory area `dest` to the constant byte `value`.
static inline void mem_set(void *dest, uint8_t value, size_t size) {
    __builtin_memset(dest, value, size);
}
