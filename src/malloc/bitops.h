#pragma once

#include <stdint.h>

#if (__has_builtin(__builtin_clz)) && !defined(SOFTBIT)
// Count Leading Zeroes
__attribute__((always_inline)) static inline int clz32(uint32_t x) {
    return x ? __builtin_clz(x) : 32;
}
#else
__attribute__((always_inline)) static inline int clz32(uint32_t x) {
    if (x == 0)
        return 32;

    int n = 0;
    if (x <= 0x0000FFFF) {
        n  += 16;
        x <<= 16;
    }
    if (x <= 0x00FFFFFF) {
        n  += 8;
        x <<= 8;
    }
    if (x <= 0x0FFFFFFF) {
        n  += 4;
        x <<= 4;
    }
    if (x <= 0x3FFFFFFF) {
        n  += 2;
        x <<= 2;
    }
    if (x <= 0x7FFFFFFF) {
        n++;
    }

    return n;
}
#endif

// Count Trailing Zeroes
#if (__has_builtin(__builtin_ctz)) && !defined(SOFTBIT)
__attribute__((always_inline)) static inline int ctz32(uint32_t x) {
    return x ? __builtin_ctz(x) : 32;
}
#else
__attribute__((always_inline)) static inline int ctz32(uint32_t x) {
    if (x == 0)
        return 32;

    static int const bitpositions[32] = {0,  1,  28, 2,  29, 14, 24, 3, 30, 22, 20, 15, 25, 17, 4,  8,
                                         31, 27, 13, 23, 21, 19, 16, 7, 26, 12, 18, 6,  11, 5,  10, 9};
    return bitpositions[((uint32_t)((x & -x) * 0x077CB531U)) >> 27];
}
#endif

// Population Count (Count set bits)
#if (__has_builtin(__builtin_popcount)) && !defined(SOFTBIT)
__attribute__((always_inline)) static inline int popcount32(uint32_t x) {
    return __builtin_popcount(x);
}
#else
__attribute__((always_inline)) static inline int popcount32(uint32_t x) {
    x = x - ((x >> 1) & 0x55555555);
    x = (x & 0x33333333) + ((x >> 2) & 0x33333333);
    x = (x + (x >> 4)) & 0x0F0F0F0F;
    x = x + (x >> 8);
    x = x + (x >> 16);
    return x & 0x3F;
}
#endif

// Find First Set bit
// Returns one plus the index of the least significant set bit of x, or if x is zero, returns zero.
#if (__has_builtin(__builtin_ffs)) && !defined(SOFTBIT)
__attribute__((always_inline)) static inline int ffs32(uint32_t x) {
    return __builtin_ffs(x);
}
#else
__attribute__((always_inline)) static inline int ffs32(uint32_t x) {
    if (x == 0)
        return 0;

    int position = 1;
    while (x != 0) {
        if (x & 1) {
            return position;
        }
        x >>= 1;
        position++;
    }
    return 0;
}
#endif

#if (__has_builtin(__builtin_clzll)) && !defined(SOFTBIT)
__attribute__((always_inline)) static inline int clz64(uint64_t x) {
    return x ? __builtin_clzll(x) : 64;
}
#else
__attribute__((always_inline)) static inline int clz64(uint64_t x) {
    if (x == 0)
        return 64;

    int n = 0;
    if (x <= 0x00000000FFFFFFFF) {
        n  += 32;
        x <<= 32;
    }
    if (x <= 0x0000FFFFFFFFFFFF) {
        n  += 16;
        x <<= 16;
    }
    if (x <= 0x00FFFFFFFFFFFFFF) {
        n  += 8;
        x <<= 8;
    }
    if (x <= 0x0FFFFFFFFFFFFFFF) {
        n  += 4;
        x <<= 4;
    }
    if (x <= 0x3FFFFFFFFFFFFFFF) {
        n  += 2;
        x <<= 2;
    }
    if (x <= 0x7FFFFFFFFFFFFFFF) {
        n++;
    }

    return n;
}
#endif

#if (__has_builtin(__builtin_clzll)) && !defined(SOFTBIT)
__attribute__((always_inline)) static inline int ctz64(uint64_t x) {
    return x ? __builtin_ctzll(x) : 64;
}
#else
__attribute__((always_inline)) static inline int ctz64(uint64_t x) {
    if (x == 0)
        return 64;

    static int const bitpositions[64] = {0,  1,  2,  53, 3,  7,  54, 27, 4,  38, 41, 8,  34, 55, 48, 28,
                                         62, 5,  39, 46, 44, 42, 22, 9,  24, 35, 59, 56, 49, 18, 29, 11,
                                         63, 52, 6,  26, 37, 40, 33, 47, 61, 45, 43, 21, 23, 58, 17, 10,
                                         51, 25, 36, 32, 60, 20, 57, 16, 50, 31, 19, 15, 30, 14, 13, 12};
    return bitpositions[((uint64_t)(x & -x) * 0x022FDD63CC95386DULL) >> 58];
}
#endif

#if (__has_builtin(__builtin_popcountll)) && !defined(SOFTBIT)
__attribute__((always_inline)) static inline int popcount64(uint64_t x) {
    return __builtin_popcountll(x);
}
#else
__attribute__((always_inline)) static inline int popcount64(uint64_t x) {
    x = x - ((x >> 1) & 0x5555555555555555);
    x = (x & 0x3333333333333333) + ((x >> 2) & 0x3333333333333333);
    x = (x + (x >> 4)) & 0x0F0F0F0F0F0F0F0F;
    x = x + (x >> 8);
    x = x + (x >> 16);
    x = x + (x >> 32);
    return x & 0x7F;
}
#endif

#if (__has_builtin(__builtin_ffsll)) && !defined(SOFTBIT)
__attribute__((always_inline)) static inline int ffs64(uint64_t x) {
    return __builtin_ffsll(x);
}
#else
__attribute__((always_inline)) static inline int ffs64(uint64_t x) {
    if (x == 0)
        return 0;

    int position = 1;
    while (x != 0) {
        if (x & 1) {
            return position;
        }
        x >>= 1;
        position++;
    }
    return 0;
}
#endif

__attribute__((always_inline)) static inline uint32_t count_set_bits64(uint64_t word) {
    return popcount64(word);
}
__attribute__((always_inline)) static inline uint32_t count_trailing_set_bits64(uint64_t word) {
    return ctz64(~word);
}
__attribute__((always_inline)) static inline uint32_t count_trailing_unset_bits64(uint64_t word) {
    return ctz64(word);
}
__attribute__((always_inline)) static inline uint32_t count_leading_set_bits64(uint64_t word) {
    return clz64(~word);
}
__attribute__((always_inline)) static inline uint32_t count_leading_unset_bits64(uint64_t word) {
    return clz64(word);
}
__attribute__((always_inline)) static inline uint32_t find_first_trailing_set_bit64(uint64_t word) {
    return ffs64(word) - 1;
}

__attribute__((always_inline)) static inline uint32_t count_set_bits32(uint32_t word) {
    return popcount32(word);
}
__attribute__((always_inline)) static inline uint32_t count_trailing_set_bits32(uint32_t word) {
    return ctz32(~word);
}
__attribute__((always_inline)) static inline uint32_t count_trailing_unset_bits32(uint32_t word) {
    return ctz32(word);
}
__attribute__((always_inline)) static inline uint32_t count_leading_set_bits32(uint32_t word) {
    return clz32(~word);
}
__attribute__((always_inline)) static inline uint32_t count_leading_unset_bits32(uint32_t word) {
    return clz32(word);
}
__attribute__((always_inline)) static inline uint32_t find_first_trailing_set_bit32(uint64_t word) {
    return ffs32(word) - 1;
}
