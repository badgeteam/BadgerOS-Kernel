
// SPDX-License-Identifier: MIT

#pragma once

// this file provides convenience macros for the attributes provided by gcc:
// https://gcc.gnu.org/onlinedocs/gcc/Common-Function-Attributes.html

// Disable address sanitization for a function.
#define NOASAN __attribute__((no_sanitize("address")))

// Declares that a function cannot return.
#define NORETURN __attribute__((noreturn))

// Declares that a function has no observable side effects and does not mutate its parameters.
#define PURE __attribute__((pure))

// Declares that a function has no observable side effects, does not mutate its parameters and does not read memory.
#define CONST __attribute__((const))

// Declares that a function is not called very often and can be size-optimized even in fast builds.
#define COLD __attribute__((cold))

// Declares that a function is called very often and should be speed-optimized even in small builds.
#define HOT __attribute__((hot))

// Declares that a function call must be inlined whenever possible.
#define FORCEINLINE __attribute__((always_inline))

// Declares that a symbol will be placed in a section called `name`
#define SECTION(name) __attribute__((section(name)))

// Declares that a symbol will be aligned to `alignment`
#define ALIGNED_TO(alignment) __attribute__((aligned(alignment)))

// Packed struct (don't add padding to align fields).
#define PACKED __attribute__((packed))

// Function written in assembly.
#define NAKED __attribute__((naked))

// Volatile variables.
#define VOLATILE volatile

// Struct is little-endian.
#define LITTLE_ENDIAN __attribute__((scalar_storage_order("little-endian")))

// Struct is big-endian.
#define BIG_ENDIAN __attribute__((scalar_storage_order("bog-endian")))
