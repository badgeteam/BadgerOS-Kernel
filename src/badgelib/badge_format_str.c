
// SPDX-License-Identifier: MIT

#include "badge_format_str.h"

#include "badge_strings.h"
#include "log.h"
#include "num_to_str.h"
#include "rawprint.h"



#if __UINT32_MAX__ == __SIZE_MAX__
#define FMT_TYPE_SIZE FMT_TYPE_INT32
#else
#define FMT_TYPE_SIZE FMT_TYPE_INT64
#endif

#if __LONG_LONG_MAX__ > __INT64_MAX__
#define FMT_TYPE_INT64 FMT_TYPE_LONG
#else
#define FMT_TYPE_INT64 FMT_TYPE_LLONG
#endif

#if __LONG_MAX__ > __INT32_MAX__
#define FMT_TYPE_INT32 FMT_TYPE_INT
#else
#define FMT_TYPE_INT32 FMT_TYPE_LONG
#endif

#if __INT_MAX__ > __INT16_MAX__
#define FMT_TYPE_INT16 FMT_TYPE_SHORT
#else
#define FMT_TYPE_INT16 FMT_TYPE_INT
#endif

#define FMT_TYPE_INT8 FMT_TYPE_CHAR



// Size of integer types.
typedef enum { FMT_TYPE_CHAR, FMT_TYPE_SHORT, FMT_TYPE_INT, FMT_TYPE_LONG, FMT_TYPE_LLONG } format_str_type_t;

// Format specifiers.
typedef enum {
    FMT_SPEC_DEC,
    FMT_SPEC_HEX,
    FMT_SPEC_OCTAL,
    FMT_SPEC_CHAR,
} format_str_spec_t;

// Multiples specifier.
typedef enum {
    FMT_MULT_ONE,
    FMT_MULT_NUL,
    FMT_MULT_ARR,
} format_str_mult_t;



#define fmt_type_size(type)                                                                                            \
    (((type) == FMT_TYPE_CHAR)    ? sizeof(char)                                                                       \
     : ((type) == FMT_TYPE_SHORT) ? sizeof(short)                                                                      \
     : ((type) == FMT_TYPE_INT)   ? sizeof(int)                                                                        \
     : ((type) == FMT_TYPE_LONG)  ? sizeof(long)                                                                       \
                                  : sizeof(long long))

#define fmt_type_index(type, arr, i)                                                                                   \
    (((type) == FMT_TYPE_CHAR)    ? ((char const *)(arr))[i]                                                           \
     : ((type) == FMT_TYPE_SHORT) ? ((short const *)(arr))[i]                                                          \
     : ((type) == FMT_TYPE_INT)   ? ((int const *)(arr))[i]                                                            \
     : ((type) == FMT_TYPE_LONG)  ? ((long const *)(arr))[i]                                                           \
                                  : ((long long const *)(arr))[i])

// Set the lowercase bit.
// Note: This is not a correct ASCII to lowercase algorithm.
#define lower(x) ((x) | 0x20)



// Try to parse a type specifier.
// This is a long function but calling a function for short string comparisons is too expensive.
static bool format_str_parse_type(char const *substr, size_t length, format_str_type_t *type_out, bool *is_signed_out) {
    // Check explicit size integers.
    if (lower(substr[0]) == 'u' || lower(substr[0]) == 'i') {
        if (length == 2 && substr[1] == '8') {
            // 8-bit integers.
            *type_out      = FMT_TYPE_INT8;
            *is_signed_out = lower(substr[0]) == 'i';
            return true;

        } else if (length == 3 && substr[1] == '1' && substr[2] == '6') {
            // 16-bit integers.
            *type_out      = FMT_TYPE_INT16;
            *is_signed_out = lower(substr[0]) == 'i';
            return true;

        } else if (length == 3 && substr[1] == '3' && substr[2] == '2') {
            // 32-bit integers.
            *type_out      = FMT_TYPE_INT32;
            *is_signed_out = lower(substr[0]) == 'i';
            return true;

        } else if (length == 3 && substr[1] == '6' && substr[2] == '4') {
            // 64-bit integers.
            *type_out      = FMT_TYPE_INT64;
            *is_signed_out = lower(substr[0]) == 'i';
            return true;
        }
    }

    // Types with implicit sign.
    if (length == 4 && lower(substr[0]) == 's' && lower(substr[1]) == 'i' && lower(substr[2]) == 'z' &&
        lower(substr[3]) == 'e') {
        // size
        *type_out      = FMT_TYPE_SIZE;
        *is_signed_out = false;
        return true;
    } else if (length == 7 && lower(substr[0]) == 'p' && lower(substr[1]) == 't' && lower(substr[2]) == 'r' &&
               lower(substr[3]) == 'd' && lower(substr[4]) == 'i' && lower(substr[5]) == 'f' &&
               lower(substr[6]) == 'f') {
        // size
        *type_out      = FMT_TYPE_SIZE;
        *is_signed_out = true;
        return true;
    }

    // Get the unsigned bit.
    bool is_signed = (lower(*substr) != 'u');
    if (!is_signed) {
        substr++, length--;
    }

    // Assert enough input data.
    if (length < 3)
        return false;

    if (length == 5 && lower(substr[0]) == 'l' && lower(substr[1]) == 'l' && lower(substr[2]) == 'o' &&
        lower(substr[3]) == 'n' && lower(substr[4]) == 'g') {
        // long long int
        *type_out      = FMT_TYPE_LLONG;
        *is_signed_out = is_signed;
        return true;

    } else if (length == 4 && lower(substr[0]) == 'l' && lower(substr[1]) == 'o' && lower(substr[2]) == 'n' &&
               lower(substr[3]) == 'g') {
        // long int
        *type_out      = FMT_TYPE_LONG;
        *is_signed_out = is_signed;
        return true;

    } else if (length == 3 && lower(substr[0]) == 'i' && lower(substr[1]) == 'n' && lower(substr[2]) == 't') {
        // int
        *type_out      = FMT_TYPE_INT;
        *is_signed_out = is_signed;
        return true;

    } else if (length == 5 && lower(substr[0]) == 's' && lower(substr[1]) == 'h' && lower(substr[2]) == 'o' &&
               lower(substr[3]) == 'r' && lower(substr[4]) == 't') {
        // long int
        *type_out      = FMT_TYPE_SHORT;
        *is_signed_out = is_signed;
        return true;

    } else if (length == 4 && lower(substr[0]) == 'c' && lower(substr[1]) == 'h' && lower(substr[2]) == 'a' &&
               lower(substr[3]) == 'r') {
        // char
        *type_out      = FMT_TYPE_CHAR;
        *is_signed_out = is_signed;
        return true;
    }

    return false;
}

// Try to parse a multiples specifier.
static bool format_str_parse_mult(char const *substr, size_t length, format_str_mult_t *mult_out) {
    if ((length == 5 && cstr_prefix_equals_case(substr, "array", 5)) ||
        (length == 3 && cstr_prefix_equals_case(substr, "arr", 3))) {
        // Pointer+length array.
        *mult_out = FMT_MULT_ARR;
        return true;

    } else if ((length == 4 && cstr_prefix_equals_case(substr, "null", 4)) ||
               (length == 3 && cstr_prefix_equals_case(substr, "nul", 3))) {
        // Null-terminated array.
        *mult_out = FMT_MULT_NUL;
        return true;
    }

    return false;
}

// Output a value given format specifiers.
bool format_str_output(
    format_str_type_t type,
    format_str_spec_t spec,
    bool              is_signed,
    bool              lowercase,
    long long         value,
    format_str_cb_t   callback,
    void             *cookie
) {
    char const hextab[] = "0123456789ABCDEF";

    if (spec == FMT_SPEC_CHAR) {
        char c = value;
        return callback(&c, 1, cookie);

    } else if (spec == FMT_SPEC_HEX) {
        char numbuf[sizeof(long long) * 2];
        char mask    = lowercase ? 0x20 : 0;
        int  n_digit = fmt_type_size(type) * 2;
        for (int i = n_digit - 1; i >= 0; i--) {
            numbuf[i]   = hextab[value & 15] | mask;
            value     >>= 4;
        }
        return callback(numbuf, n_digit, cookie);

    } else if (spec == FMT_SPEC_OCTAL) {
        char numbuf[(sizeof(long long) * 8 - 1) / 3 + 1];
        int  n_digit = (int)(fmt_type_size(type) * 8 - 1) / 3 + 1;
        for (int i = n_digit - 1; i >= 0; i--) {
            numbuf[i]   = '0' | (value & 7);
            value     >>= 3;
        }
        return callback(numbuf, n_digit, cookie);

    } else if (spec == FMT_SPEC_DEC) {
        char   numbuf[21];
        size_t n_digit;
        if (is_signed) {
            n_digit = int_to_cstr_packed(value, numbuf, sizeof(numbuf));
        } else {
            n_digit = uint_to_cstr_packed(value, numbuf, sizeof(numbuf));
        }
        return callback(numbuf, n_digit, cookie);
    }

    return false;
}

// Format a string and output characters via callback.
bool format_str_va(char const *msg, size_t length, format_str_cb_t callback, void *cookie, va_list vararg) {
    // Current read index.
    size_t i = 0;

    while (i < length) {
        // Check for the escape character.
        if (msg[i] == '%' && i + 1 < length && msg[i + 1] == '%') {
            // A literal '%'.
            if (!callback("%", 1, cookie))
                return false;
            i += 2;

        } else if (msg[i] == '%' && i + 3 < length) {
            // A format specifier.
            // Search for the brackets.
            ptrdiff_t fmt_end = mem_index(&msg[i], length - i, '}');
            if (msg[i + 1] != '{' || fmt_end == -1) {
                goto illegal_format;
            }
            fmt_end += (ptrdiff_t)i;

            // Format specifiers to decode:
            format_str_type_t type       = FMT_TYPE_INT;
            format_str_spec_t spec       = FMT_SPEC_DEC;
            format_str_mult_t mult       = FMT_MULT_ONE;
            bool              lowercase  = true;
            bool              is_signed  = true;
            char const       *joiner     = NULL;
            size_t            joiner_len = 0;

            // Check for the aliases.
            if ((size_t)fmt_end == i + 4 && lower(msg[i + 2]) == 'c' && lower(msg[i + 3]) == 's') {
                // C-string.
                type       = FMT_TYPE_CHAR;
                spec       = FMT_SPEC_CHAR;
                mult       = FMT_MULT_NUL;
                joiner     = NULL;
                joiner_len = 0;
                is_signed  = false;

            } else if ((size_t)fmt_end == i + 4 && lower(msg[i + 2]) == 'l' && lower(msg[i + 3]) == 's') {
                // Pointer+length string.
                type       = FMT_TYPE_CHAR;
                spec       = FMT_SPEC_CHAR;
                mult       = FMT_MULT_ARR;
                joiner     = NULL;
                joiner_len = 0;
                is_signed  = false;

            } else {
                char const *substr = &msg[i + 2];
                size_t      sublen = fmt_end - i - 2;

                // Check for the presence of a type specifier.
                ptrdiff_t delim = mem_index(substr, sublen, ';');
                if (delim < 0) {
                    type      = FMT_TYPE_INT;
                    is_signed = true;
                } else if (delim != 1 && format_str_parse_type(substr, delim, &type, &is_signed)) {
                    substr += delim + 1;
                    sublen -= delim + 1;
                } else if (delim != 1) {
                    goto illegal_format;
                }

                // Get the format specifier.
                if (!sublen)
                    goto illegal_format;
                lowercase = substr[0] & 0x20;
                switch (lower(substr[0])) {
                    case 'd': spec = FMT_SPEC_DEC; break;
                    case 'x': spec = FMT_SPEC_HEX; break;
                    case 'o':
                    case 'q': spec = FMT_SPEC_OCTAL; break;
                    case 'c': spec = FMT_SPEC_CHAR; break;
                    default: goto illegal_format;
                }

                // Check for the presence of a multiples specifier.
                delim = mem_index(substr, sublen, ';');
                if (delim < 0 && sublen != 1) {
                    goto illegal_format;
                } else if (delim < 0) {
                    mult = FMT_MULT_ONE;
                } else {
                    substr += delim + 1;
                    sublen -= delim + 1;
                    delim   = mem_index(substr, sublen, ';');
                    if (delim >= 0 && format_str_parse_mult(substr, delim, &mult)) {
                        joiner     = &substr[delim + 1];
                        joiner_len = sublen - delim - 1;
                    } else if (delim < 0 && format_str_parse_mult(substr, sublen, &mult)) {
                        joiner     = NULL;
                        joiner_len = 0;
                    } else {
                        goto illegal_format;
                    }
                }
            }

            if (mult == FMT_MULT_ARR && spec == FMT_SPEC_CHAR && type == FMT_TYPE_CHAR && !joiner) {
                // Length-string optimisation.
                char const *cstr   = va_arg(vararg, void const *);
                size_t      length = va_arg(vararg, size_t);
                if (!callback(cstr, length, cookie))
                    return false;

            } else if (mult == FMT_MULT_ARR) {
                // Get pointer.
                void const *ptr    = va_arg(vararg, void const *);
                // Get length.
                size_t      length = va_arg(vararg, size_t);
                // Print some stuff.
                for (size_t i = 0; i < length; i++) {
                    if (i && !callback(joiner, joiner_len, cookie)) {
                        return false;
                    }
                    if (!format_str_output(
                            type,
                            spec,
                            is_signed,
                            lowercase,
                            fmt_type_index(type, ptr, i),
                            callback,
                            cookie
                        ))
                        return false;
                }

            } else if (mult == FMT_MULT_NUL && spec == FMT_SPEC_CHAR && type == FMT_TYPE_CHAR && !joiner) {
                // C-string optimisation.
                char const *cstr = va_arg(vararg, void const *);
                if (!callback(cstr, cstr_length(cstr), cookie))
                    return false;

            } else if (mult == FMT_MULT_NUL) {
                // Get pointer.
                void const *ptr = va_arg(vararg, void const *);
                // Print some stuff.
                for (size_t i = 0;; i++) {
                    long long value = fmt_type_index(type, ptr, i);
                    if (!value)
                        break;
                    if (i && joiner && !callback(joiner, joiner_len, cookie)) {
                        return false;
                    }
                    if (!format_str_output(type, spec, is_signed, lowercase, value, callback, cookie))
                        return false;
                }

            } else {
                long long value = 0;
                switch (type) {
                    case FMT_TYPE_CHAR:
                        if (is_signed)
                            value = (long long)(signed char)va_arg(vararg, int);
                        else
                            value = (long long)(unsigned char)va_arg(vararg, unsigned int);
                        break;

                    case FMT_TYPE_SHORT:
                        if (is_signed)
                            value = (long long)(short)va_arg(vararg, int);
                        else
                            value = (long long)(unsigned short)va_arg(vararg, unsigned int);
                        break;

#if __LONG_MAX__ == __INT_MAX__
                    case FMT_TYPE_LONG:
#endif
                    case FMT_TYPE_INT:
                        if (is_signed) // NOLINT
                            value = va_arg(vararg, int);
                        else
                            value = va_arg(vararg, unsigned int);
                        break;

#if __LONG_MAX__ == __LONG_LONG_MAX__
                    case FMT_TYPE_LONG:
#endif
                    case FMT_TYPE_LLONG:
                        if (is_signed)
                            value = va_arg(vararg, long long);
                        else
                            value = va_arg(vararg, unsigned long long);
                        break;
                }
                if (!format_str_output(type, spec, is_signed, lowercase, value, callback, cookie))
                    return false;
            }

            i = fmt_end + 1;

        } else {
            // Normal text.
            size_t start = i;
            while (i < length && msg[i] != '%') i++;
            if (!callback(&msg[start], i - start, cookie))
                return false;
        }
    }

    return true;

// When the format specifier is malformed, the rest is printed verbatim.
illegal_format:
    return callback(&msg[i], length - i, cookie);
}
