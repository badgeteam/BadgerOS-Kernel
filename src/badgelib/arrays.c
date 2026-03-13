
// SPDX-License-Identifier: MIT

#include "arrays.h"

#include "assertions.h"
#include "badge_strings.h"
#include "malloc.h"



// Get the address of an entry.
static inline void *array_index(void *array, size_t ent_size, size_t index) {
    return (void *)((size_t)array + ent_size * index);
}

// Get the address of an entry.
static inline void const *array_index_const(void const *array, size_t ent_size, size_t index) {
    return (void *)((size_t)array + ent_size * index);
}

// Swap two elements.
static inline void array_swap(void *array, size_t ent_size, size_t a, size_t b) {
    mem_swap(array_index(array, ent_size, a), array_index(array, ent_size, b), ent_size);
}



// Binary search for a value in a sorted (ascending order) array.
array_binsearch_t array_binsearch(
    void const *array, size_t ent_size, size_t ent_count, void const *value, array_sort_comp_t comparator
) {
    size_t ent_start = 0;
    while (ent_count > 0) {
        size_t midpoint = ent_count >> 1;
        int    res      = comparator(array_index_const(array, ent_size, midpoint), value);
        if (res > 0) {
            // The value is to the left of the midpoint.
            ent_count = midpoint;

        } else if (res < 0) {
            // The value is to the right of the midpoint.
            ent_start += midpoint + 1;
            ent_count -= midpoint + 1;
            array      = array_index_const(array, ent_size, midpoint + 1);

        } else /* res == 0 */ {
            // The value was found.
            return (array_binsearch_t){ent_start + midpoint, true};
        }
    }
    // The value was not found.
    return (array_binsearch_t){ent_start, false};
}



// A recursive array sorting implementation.
static void array_sort_impl(void *array, void *tmp, size_t ent_size, size_t ent_count, array_sort_comp_t comparator) {
    if (ent_count < 2) {
        // Limit case: 1 entry; no sorting needed.
        return;
    } else if (ent_count == 2) {
        // Limit case: 2 entries; single compare.
        if (comparator(array, (void const *)((size_t)array + ent_size)) > 0) {
            // Swap the entries.
            array_swap(array, ent_size, 0, 1);
        }
        return;
    }

    // Split array and sort recursively.
    array_sort_impl(array, tmp, ent_size, ent_count / 2, comparator);
    array_sort_impl(array_index(array, ent_size, ent_count / 2), tmp, ent_size, ent_count - ent_count / 2, comparator);

    // Merge the sorted subarrays.
    size_t avl_a = ent_count / 2;
    size_t avl_b = ent_count - ent_count / 2;
    size_t i = 0, a = 0, b = 0;

    // Continually select the lower value from either array.
    for (; a < avl_a && b < avl_b; i++) {
        void *a_ptr = array_index(array, ent_size, a);
        void *b_ptr = array_index(array, ent_size, avl_a + b);
        if (comparator(a_ptr, b_ptr) > 0) {
            mem_copy(array_index(tmp, ent_size, i), b_ptr, ent_size);
            b++;
        } else {
            mem_copy(array_index(tmp, ent_size, i), a_ptr, ent_size);
            a++;
        }
    }

    // Add any remainder.
    for (; a < avl_a; a++, i++) {
        void *a_ptr = array_index(array, ent_size, a);
        mem_copy(array_index(tmp, ent_size, i), a_ptr, ent_size);
    }
    for (; b < avl_b; b++, i++) {
        void *b_ptr = array_index(array, ent_size, avl_a + b);
        mem_copy(array_index(tmp, ent_size, i), b_ptr, ent_size);
    }

    // Copy it all back into the output.
    mem_copy(array, tmp, ent_size * ent_count);
}

// Sort a contiguous array given a comparator function.
// The array is sorted into ascending order.
void array_sort(void *array, size_t ent_size, size_t ent_count, array_sort_comp_t comparator) {
    if (ent_count <= 1) {
        // Edge case: 0 or 1 entries; no sorting needed.
    } else if (ent_count == 2) {
        // Edge case: 2 entries; single compare.
        if (comparator(array, (void const *)((size_t)array + ent_size)) > 0) {
            array_swap(array, ent_size, 0, 1);
        }
    } else {
        // 3 or more entries: sorting needed.
        void *mem = malloc(ent_size * ent_count);
        assert_always(mem != NULL);
        array_sort_impl(array, mem, ent_size, ent_count, comparator);
        free(mem);
    }
}
