
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// A meta-programming macro to obtain a pointer to `Container` from
// a pointer to `field_name`.
#define field_parent_ptr(Container, field_name, pointer_to_field)                                                      \
    ((Container *)(((char *)(pointer_to_field)) - offsetof(Container, field_name)))

// Takes whatever `value` is and returns it as a string literal
#define comptime_stringify(value) #value

// Returns the inner `value` of the given macro as a string. Can be used to
// convert things like numbers or `__LINE__` to string form.
#define convert_macro_to_string(value) comptime_stringify(value)

// Concatenate two keywords.
#define concat_keywords(a, b) a##b
