
// SPDX-License-Identifier: MIT

#include <badge_strings.h>
#include <malloc.h>



// Find the first occurrance `value` in C-string `string`.
// Returns -1 when not found.
ptrdiff_t cstr_index(char const *string, char value) {
    char const *ptr = string;

    // Find first index.
    while (*ptr) {
        if (*ptr == value)
            return ptr - string;
        ptr++;
    }

    return -1;
}

// Find the last occurrance `value` in C-string `string`.
// Returns -1 when not found.
ptrdiff_t cstr_last_index(char const *string, char value) {
    return cstr_last_index_upto(string, value, SIZE_MAX);
}

// Find the first occurrance `value` in C-string `string` from and including `first_index`.
// Returns -1 when not found.
ptrdiff_t cstr_index_from(char const *string, char value, size_t first_index) {
    char const *ptr = string + first_index;

    // Make sure string is long enough.
    for (size_t i = 0; i < first_index; i++) {
        if (!string[i])
            return -1;
    }

    // Find first index.
    while (*ptr) {
        if (*ptr == value)
            return ptr - string;
        ptr++;
    }

    return -1;
}

// Find the last occurrance `value` in C-string `string` up to and excluding `last_index`.
// Returns -1 when not found.
ptrdiff_t cstr_last_index_upto(char const *string, char value, size_t last_index) {
    ptrdiff_t found = -1;
    for (size_t i = 0; string[i] && i < last_index; i++) {
        if (string[i] == value)
            found = (ptrdiff_t)i;
    }
    return found;
}



// Duplicate a C-string.
char *cstr_duplicate(char const *a) {
    size_t len = cstr_length(a);
    char  *ptr = malloc(len + 1);
    if (ptr) {
        mem_copy(ptr, a, len);
        ptr[len] = 0;
    }
    return ptr;
}



// Test the equality of two C-strings.
int cstr_compare(char const *a, char const *b) {
    while (1) {
        if (*a != *b)
            return *a - *b;
        if (!*a)
            return 0;
        a++, b++;
    }
}

// Test the of the first `length` characters equality of two C-strings.
int cstr_prefix_compare(char const *a, char const *b, size_t length) {
    while (length--) {
        if (*a != *b)
            return *a - *b;
        if (!*a)
            return 0;
        a++, b++;
    }
    return 0;
}

// Test the equality of two C-strings, case-insensitive.
int cstr_compare_case(char const *a, char const *b) {
    while (1) {
        if (ascii_char_to_lower(*a) != ascii_char_to_lower(*b))
            return ascii_char_to_lower(*a) - ascii_char_to_lower(*b);
        if (!*a)
            return 0;
        a++, b++;
    }
}

// Test the of the first `length` characters equality of two C-strings, case-insensitive.
int cstr_prefix_compare_case(char const *a, char const *b, size_t length) {
    while (length--) {
        if (ascii_char_to_lower(*a) != ascii_char_to_lower(*b))
            return ascii_char_to_lower(*a) - ascii_char_to_lower(*b);
        if (!*a)
            return 0;
        a++, b++;
    }
    return 0;
}



// Test the equality of two C-strings.
bool cstr_equals(char const *a, char const *b) {
    return cstr_compare(a, b) == 0;
}

// Test the of the first `length` characters equality of two C-strings.
bool cstr_prefix_equals(char const *a, char const *b, size_t length) {
    return cstr_prefix_compare(a, b, length) == 0;
}

// Test the equality of two C-strings, case-insensitive.
bool cstr_equals_case(char const *a, char const *b) {
    return cstr_compare_case(a, b) == 0;
}

// Test the of the first `length` characters equality of two C-strings, case-insensitive.
bool cstr_prefix_equals_case(char const *a, char const *b, size_t length) {
    return cstr_prefix_compare_case(a, b, length) == 0;
}



// Concatenate a NULL-terminated C-string from `src` onto C-string buffer `dest`.
// This may truncate characters, but not the NULL terminator, if `dest` does not fit `src` entirely.
size_t cstr_concat(char *dest, size_t size, char const *src) {
    size_t dest_len = cstr_length(dest);
    if (dest_len < size - 1) {
        return cstr_copy(dest + dest_len, size - dest_len, src) + dest_len;
    }
    return dest_len;
}

// Concatenate a NULL-terminated C-string from `src` onto C-string buffer `dest`.
// This may truncate characters, but not the NULL terminator, if `dest` does not fit `src` entirely.
size_t cstr_concat_packed(char *dest, size_t size, char const *src) {
    size_t dest_len = cstr_length_upto(dest, size);
    if (dest_len < size) {
        return cstr_copy_packed(dest + dest_len, size - dest_len, src) + dest_len;
    }
    return dest_len;
}

// Copy a NULL-terminated C-string from `src` into buffer `dest`.
// This may truncate characters, but not the NULL terminator, if `dest` does not fit `src` entirely.
size_t cstr_copy(char *dest, size_t size, char const *src) {
    char const *const orig = dest;
    while (size > 1) {
        if (!*src)
            break;
        *dest = *src;
        dest++, src++;
        size--;
    }
    if (size) {
        *dest = 0;
    }
    return dest - orig;
}

// Copy at most `length` bytes C-string `src` into buffer `dest`.
// WARNING: This may leave strings without NULL terminators if `dest` does not fit `src` entirely.
size_t cstr_copy_packed(char *dest, size_t size, char const *src) {
    char const *const orig = dest;
    while (size--) {
        *dest = *src;
        if (!*src)
            return dest - orig;
        dest++, src++;
    }
    return dest - orig;
}



// Find the first occurrance of byte `value` in memory `memory`.
// Returns -1 when not found.
ptrdiff_t mem_index(void const *memory, size_t size, uint8_t value) {
    uint8_t const *ptr = memory;
    for (size_t i = 0; i < size; i++) {
        if (ptr[i] == value)
            return (ptrdiff_t)i;
    }
    return -1;
}

// Find the first occurrance of byte `value` in memory `memory`.
// Returns -1 when not found.
ptrdiff_t mem_last_index(void const *memory, size_t size, uint8_t value) {
    uint8_t const *ptr = memory;
    for (size_t i = size; i-- > 0;) {
        if (ptr[i] == value)
            return (ptrdiff_t)i;
    }
    return -1;
}

// Implementation of the mem_equals loop with variable read size.
#define MEM_EQUALS_IMPL(type, alignment, a, b, size)                                                                   \
    {                                                                                                                  \
        type const *a_ptr = (a); /* NOLINT*/                                                                           \
        type const *b_ptr = (b); /* NOLINT*/                                                                           \
        size_t      _size = (size) / (alignment);                                                                      \
        for (size_t i = 0; i < _size; i++) {                                                                           \
            if (a_ptr[i] != b_ptr[i])                                                                                  \
                return false;                                                                                          \
        }                                                                                                              \
    }

// Test the equality of two memory areas.
bool mem_equals(void const *a, void const *b, size_t size) {
    size_t align_detector = (size_t)a | (size_t)b | (size_t)size;

    // Optimise for alignment.
    if (align_detector & 1) {
        MEM_EQUALS_IMPL(uint8_t, 1, a, b, size)
    } else if (align_detector & 2) {
        MEM_EQUALS_IMPL(uint16_t, 2, a, b, size)
    } else if (align_detector & 4) {
        MEM_EQUALS_IMPL(uint32_t, 4, a, b, size)
    } else {
        MEM_EQUALS_IMPL(uint64_t, 8, a, b, size)
    }

    return true;
}


// Implementation of the mem_swap loop with variable access size.
#define MEM_SWAP_IMPL(type, alignment, a, b, size)                                                                     \
    {                                                                                                                  \
        type  *a_ptr = (a); /* NOLINT*/                                                                                \
        type  *b_ptr = (b); /* NOLINT*/                                                                                \
        size_t _size = (size) / (alignment);                                                                           \
        type   tmp;                                                                                                    \
        for (size_t i = 0; i < _size; i++) {                                                                           \
            tmp      = a_ptr[i];                                                                                       \
            a_ptr[i] = b_ptr[i];                                                                                       \
            b_ptr[i] = tmp;                                                                                            \
        }                                                                                                              \
    }

// Swap the contents of memory areas `a` and `b`.
// For correct copying, `a` and `b` must not overlap.
void mem_swap(void *a, void *b, size_t size) {
    size_t align_detector = (size_t)a | (size_t)b | (size_t)size;

    // Optimise for alignment.
    if (align_detector & 1) {
        MEM_SWAP_IMPL(uint8_t, 1, a, b, size)
    } else if (align_detector & 2) {
        MEM_SWAP_IMPL(uint16_t, 2, a, b, size)
    } else if (align_detector & 4) {
        MEM_SWAP_IMPL(uint32_t, 4, a, b, size)
    } else {
        MEM_SWAP_IMPL(uint64_t, 8, a, b, size)
    }
}
