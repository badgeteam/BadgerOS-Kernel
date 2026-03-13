
// SPDX-License-Identifier: MIT

#include "page_alloc.h"

#include "badge_strings.h"
#include "static-buddy.h"

#if !CONFIG_NOMMU
#include "mem/vmm.h"
#endif



// Allocate pages of physical memory.
// Uses physical page numbers (paddr / CONFIG_PAGE_SIZE).
size_t phys_page_alloc(size_t page_count, bool for_user) {
    void *mem = buddy_allocate(page_count * CONFIG_PAGE_SIZE, for_user ? BLOCK_TYPE_USER : BLOCK_TYPE_PAGE, 0);
    if (!mem) {
        return 0;
    }
    mem_set(mem, 0, page_count * CONFIG_PAGE_SIZE);
#if !CONFIG_NOMMU
    return ((size_t)mem - vmm_hhdm_offset) / CONFIG_PAGE_SIZE;
#else
    return (size_t)mem / CONFIG_PAGE_SIZE;
#endif
}

// Returns how large a physical allocation actually is.
// Uses physical page numbers (paddr / CONFIG_PAGE_SIZE).
size_t phys_page_size(size_t ppn) {
#if !CONFIG_NOMMU
    return buddy_get_size((void *)(ppn * CONFIG_PAGE_SIZE + vmm_hhdm_offset)) / CONFIG_PAGE_SIZE;
#else
    return buddy_get_size((void *)(ppn * CONFIG_PAGE_SIZE)) / CONFIG_PAGE_SIZE;
#endif
}

// Free pages of physical memory.
// Uses physical page numbers (paddr / CONFIG_PAGE_SIZE).
void phys_page_free(size_t ppn) {
#if !CONFIG_NOMMU
    buddy_deallocate((void *)(ppn * CONFIG_PAGE_SIZE + vmm_hhdm_offset));
#else
    buddy_deallocate((void *)(ppn * CONFIG_PAGE_SIZE));
#endif
}
