
// SPDX-License-Identifier: MIT

// This file contains definitions for all GCC routines for integer arithmetic with the exception of int bit shifts.

// NOLINTBEGIN

#pragma GCC optimize("O2")

#include <stdbool.h>
#include <stdint.h>


typedef int          si_t __attribute__((mode(SI)));
typedef unsigned int usi_t __attribute__((mode(SI)));
typedef int          di_t __attribute__((mode(DI)));
typedef unsigned int udi_t __attribute__((mode(DI)));
#if __SIZE_MAX__ > 0xffffffff
#define do_ti_math
typedef int          ti_t __attribute__((mode(TI)));
typedef unsigned int uti_t __attribute__((mode(TI)));
#endif


// Implement division and modulo for an unsigned type.
// Does not do any edge case checks; divisor must never be 0.
#define DIVMOD_IMPL(type, name)                                                                                        \
    typedef struct {                                                                                                   \
        u##type remainder;                                                                                             \
        u##type result;                                                                                                \
    } divmod_##name##_t;                                                                                               \
    static divmod_##name##_t divmod_##name(u##type remainder, u##type divisor) {                                       \
        u##type msb           = 1;                                                                                     \
        msb                 <<= (sizeof(type) * 8 - 1);                                                                \
        u##type      result   = 0;                                                                                     \
        unsigned int shift    = 0;                                                                                     \
        while (!(divisor & msb)) {                                                                                     \
            divisor <<= 1;                                                                                             \
            shift++;                                                                                                   \
        }                                                                                                              \
        for (unsigned int i = 0; i <= shift; i++) {                                                                    \
            if (remainder >= divisor) {                                                                                \
                result    |= 1ull << (shift - i);                                                                      \
                remainder -= divisor;                                                                                  \
            }                                                                                                          \
            divisor >>= 1;                                                                                             \
        }                                                                                                              \
        return (divmod_##name##_t){remainder, result};                                                                 \
    }

// Division implementations.
DIVMOD_IMPL(si_t, si)
DIVMOD_IMPL(di_t, di)
#ifdef do_ti_math
DIVMOD_IMPL(ti_t, ti)
#endif

// Generate all division functions for a type.
#define DIVMOD_FUNCS(type, name)                                                                                       \
    u##type __udivmod##name##4(u##type a, u##type b, u##type * rem) {                                                  \
        if (b == 0) {                                                                                                  \
            *rem = a;                                                                                                  \
            return -1;                                                                                                 \
        }                                                                                                              \
        divmod_##name##_t res = divmod_##name(a, b);                                                                   \
        *rem                  = res.remainder;                                                                         \
        return res.result;                                                                                             \
    }                                                                                                                  \
    u##type __udiv##name##3(u##type a, u##type b) {                                                                    \
        if (b == 0)                                                                                                    \
            return -1;                                                                                                 \
        return divmod_##name(a, b).result;                                                                             \
    }                                                                                                                  \
    u##type __umod##name##3(u##type a, u##type b) {                                                                    \
        if (b == 0)                                                                                                    \
            return a;                                                                                                  \
        else                                                                                                           \
            return divmod_##name(a, b).remainder;                                                                      \
    }                                                                                                                  \
    type __divmod##name##4(type a, type b, type * rem) {                                                               \
        if (b == 0) {                                                                                                  \
            *rem = a;                                                                                                  \
            return -1;                                                                                                 \
        }                                                                                                              \
        bool              div_sign = (a < 0) ^ (b < 0);                                                                \
        bool              mod_sign = a < b;                                                                            \
        divmod_##name##_t res      = divmod_##name(a, b);                                                              \
        *rem                       = mod_sign ? -res.remainder : res.remainder;                                        \
        return div_sign ? -res.result : res.result;                                                                    \
    }                                                                                                                  \
    type __div##name##3(type a, type b) {                                                                              \
        if (b == 0)                                                                                                    \
            return -1;                                                                                                 \
        bool sign = (a < 0) ^ (b < 0);                                                                                 \
        if (a < 0)                                                                                                     \
            a = -a;                                                                                                    \
        if (b < 0)                                                                                                     \
            b = -b;                                                                                                    \
        divmod_##name##_t res = divmod_##name(a, b);                                                                   \
        if (sign)                                                                                                      \
            return -res.result;                                                                                        \
        else                                                                                                           \
            return res.result;                                                                                         \
    }                                                                                                                  \
    type __mod##name##3(type a, type b) {                                                                              \
        if (b == 0)                                                                                                    \
            return a;                                                                                                  \
        bool sign = a < 0;                                                                                             \
        if (a < 0)                                                                                                     \
            a = -a;                                                                                                    \
        if (b < 0)                                                                                                     \
            b = -b;                                                                                                    \
        divmod_##name##_t res = divmod_##name(a, b);                                                                   \
        if (sign)                                                                                                      \
            return -res.remainder;                                                                                     \
        else                                                                                                           \
            return res.remainder;                                                                                      \
    }

DIVMOD_FUNCS(si_t, si)
DIVMOD_FUNCS(di_t, di)
#ifdef do_ti_math
DIVMOD_FUNCS(ti_t, ti)
#endif


// Fake integer operations by letting GCC write them all out itself.
// This file is O2 optimised, so it should emit assembly for all of these functions.
#define FAKE_OPER(type, name, oper)                                                                                    \
    type name(type a, type b) {                                                                                        \
        return a oper(b & (sizeof(type) * 8 - 1));                                                                     \
    }

FAKE_OPER(si_t, __ashlsi3, <<)
FAKE_OPER(si_t, __ashrsi3, >>)
FAKE_OPER(si_t, __lshrsi3, >>)
FAKE_OPER(di_t, __ashldi3, <<)
FAKE_OPER(di_t, __ashrdi3, >>)
FAKE_OPER(di_t, __lshrdi3, >>)
#ifdef do_ti_math
FAKE_OPER(ti_t, __ashlti3, <<)
FAKE_OPER(ti_t, __ashrti3, >>)
FAKE_OPER(ti_t, __lshrti3, >>)
#endif

// Explicit implementation of integer multiply for the larger versions to rely upon if required.
int __mulsi3(int a, int b) {
    int res = 0;
    for (unsigned int i = 0; i < sizeof(int) * 8; i++) {
        if (b & 1) {
            res += a;
        }
        b >>= 1;
        a <<= 1;
    }
    return res;
}

FAKE_OPER(di_t, __muldi3, *)
#ifdef do_ti_math
FAKE_OPER(ti_t, __multi3, *)
#endif


#define BSWP_FUNC(type, name)                                                                                          \
    type name(type a) {                                                                                                \
        union {                                                                                                        \
            type packed;                                                                                               \
            char arr[sizeof(type)];                                                                                    \
        } in;                                                                                                          \
        union {                                                                                                        \
            type packed;                                                                                               \
            char arr[sizeof(type)];                                                                                    \
        } out;                                                                                                         \
        in.packed = a;                                                                                                 \
        for (unsigned i = 0; i < sizeof(type); i++) {                                                                  \
            out.arr[sizeof(type) - i - 1] = in.arr[i];                                                                 \
        }                                                                                                              \
        return out.packed;                                                                                             \
    }

BSWP_FUNC(si_t, __bswapsi2)
BSWP_FUNC(di_t, __bswapdi2)
#ifdef do_ti_math
BSWP_FUNC(ti_t, __bswapti2)
#endif


// The `__clz*` count leading zero functions count how many zeroes are present, starting at the MSB.
// They first convert a number into a bitmask where only bit above the most significant set bit is set.
// This is then multiplied with a de Bruijn sequence to get a unique index in the most significant bits.
// This index is then used to read from a hash table that uniquely identifies how many leading zeroes exist.

int __clzsi2(uint32_t a) __attribute__((weak));
int __clzsi2(uint32_t a) {
    static uint8_t const hash_table[32] = {
        0, 31, 9, 30, 3, 8,  13, 29, 2,  5,  7,  21, 12, 24, 28, 19,
        1, 10, 4, 14, 6, 22, 25, 20, 11, 15, 23, 26, 16, 27, 17, 18,
    };
    for (uint32_t i = 1; i < 32; i *= 2) {
        a |= a >> i;
    }
    a++;
    return hash_table[(a * 0x076be629) >> 27];
}
int __clzdi2(uint64_t a) __attribute__((weak));
int __clzdi2(uint64_t a) {
    static uint8_t const hash_table[64] = {
        0,  63, 62, 57, 61, 51, 56, 45, 60, 39, 50, 36, 55, 30, 44, 24, 59, 47, 38, 26, 49, 18,
        35, 16, 54, 33, 29, 10, 43, 14, 23, 7,  1,  58, 52, 46, 40, 37, 31, 25, 48, 27, 19, 17,
        34, 11, 15, 8,  2,  53, 41, 32, 28, 20, 12, 9,  3,  42, 21, 13, 4,  22, 5,  6,
    };
    for (uint64_t i = 1; i < 64; i *= 2) {
        a |= a >> i;
    }
    a++;
    return hash_table[(a * 0x0218a392cd3d5dbf) >> 58];
}
#ifdef do_ti_math
int __clzti2(__uint128_t a) __attribute__((weak));
int __clzti2(__uint128_t a) {
    static uint8_t const hash_table[128] = {
        0,   127, 126, 120, 125, 113, 119, 106, 124, 99,  112, 92,  118, 85, 105, 78, 123, 95, 98,  71, 111, 64,
        91,  57,  117, 68,  84,  50,  104, 43,  77,  36,  122, 108, 94,  80, 97,  59, 70,  38, 110, 61, 63,  29,
        90,  27,  56,  22,  116, 88,  67,  46,  83,  25,  49,  15,  103, 54, 42,  12, 76,  20, 35,  8,  1,   121,
        114, 107, 100, 93,  86,  79,  96,  72,  65,  58,  69,  51,  44,  37, 109, 81, 60,  39, 62,  30, 28,  23,
        89,  47,  26,  16,  55,  13,  21,  9,   2,   115, 101, 87,  73,  66, 52,  45, 82,  40, 31,  24, 48,  17,
        14,  10,  3,   102, 74,  53,  41,  32,  18,  11,  4,   75,  33,  19, 5,   34, 6,   7,
    };
    for (__uint128_t i = 1; i < 128; i *= 2) {
        a |= a >> i;
    }
    a++;
    __uint128_t mul = ((__uint128_t)0x0106143891634793 << 64) | 0x2a5cd9d3ead7b77f;
    return hash_table[(a * mul) >> 121];
}
#endif

int __ctzsi2(uint32_t a) __attribute__((weak));
int __ctzsi2(uint32_t a) {
    static uint8_t const hash_table[32] = {
        0,  1,  23, 2,  29, 24, 19, 3,  30, 27, 25, 11, 20, 8, 4,  13,
        31, 22, 28, 18, 26, 10, 7,  12, 21, 17, 9,  6,  16, 5, 15, 14,
    };
    a &= ~(a - 1);
    return hash_table[(a * 0x076be629) >> 27];
}
int __ctzdi2(uint64_t a) __attribute__((weak));
int __ctzdi2(uint64_t a) {
    static uint8_t const hash_table[64] = {
        0,  1,  2,  7,  3,  13, 8,  19, 4,  25, 14, 28, 9,  34, 20, 40, 5,  17, 26, 38, 15, 46,
        29, 48, 10, 31, 35, 54, 21, 50, 41, 57, 63, 6,  12, 18, 24, 27, 33, 39, 16, 37, 45, 47,
        30, 53, 49, 56, 62, 11, 23, 32, 36, 44, 52, 55, 61, 22, 43, 51, 60, 42, 59, 58,
    };
    a &= ~(a - 1);
    return hash_table[(a * 0x0218a392cd3d5dbf) >> 58];
}
#ifdef do_ti_math
int __ctzti2(__uint128_t a) __attribute__((weak));
int __ctzti2(__uint128_t a) {
    static uint8_t const hash_table[128] = {
        0,   1,   2,   8,   3,  15,  9,   22,  4,   29,  16,  36,  10, 43,  23,  50,  5,   33,  30, 57,  17,  64,
        37,  71,  11,  60,  44, 78,  24,  85,  51,  92,  6,   20,  34, 48,  31,  69,  58,  90,  18, 67,  65,  99,
        38,  101, 72,  106, 12, 40,  61,  82,  45,  103, 79,  113, 25, 74,  86,  116, 52,  108, 93, 120, 127, 7,
        14,  21,  28,  35,  42, 49,  32,  56,  63,  70,  59,  77,  84, 91,  19,  47,  68,  89,  66, 98,  100, 105,
        39,  81,  102, 112, 73, 115, 107, 119, 126, 13,  27,  41,  55, 62,  76,  83,  46,  88,  97, 104, 80,  111,
        114, 118, 125, 26,  54, 75,  87,  96,  110, 117, 124, 53,  95, 109, 123, 94,  122, 121,
    };
    a               &= ~(a - 1);
    __uint128_t mul  = ((__uint128_t)0x0106143891634793 << 64) | 0x2a5cd9d3ead7b77f;
    return hash_table[(a * mul) >> 121];
}
#endif

int __ffsdi2(uint64_t a) __attribute__((weak));
int __ffsdi2(uint64_t a) {
    return a ? __ctzdi2(a) + 1 : 0;
}

#ifdef do_ti_math
int __ffsti2(__uint128_t a) __attribute__((weak));
int __ffsti2(__uint128_t a) {
    return a ? __ctzti2(a) + 1 : 0;
}
#endif

// NOLINTEND
