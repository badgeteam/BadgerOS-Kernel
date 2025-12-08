
// SPDX-License-Identifier: MIT

#include "arrays.h"
#include "assertions.h"
#include "badge_strings.h"
#include "log.h"
#include "mem/mmu.h"
#include "mem/vmm.h"
#include "page_alloc.h"
#include "panic.h"
#include "process/internal.h"
#include "process/process.h"
#include "process/types.h"
#include "scheduler/cpu.h"
#include "scheduler/types.h"
#include "todo.h"

#if !CONFIG_NOMMU
#include "cpu/mmu.h"
#ifdef PROC_MEMMAP_MAX_REGIONS
#error "When virtual memory is enabled, there must be no memmap region limit (PROC_MEMMAP_MAX_REGIONS)"
#endif
#endif

// Memory map address comparator.
static int proc_memmap_cmp(void const *a, void const *b) {
    proc_memmap_ent_t const *a_ptr = a;
    proc_memmap_ent_t const *b_ptr = b;
#if !CONFIG_NOMMU
    if (a_ptr->vaddr < b_ptr->vaddr)
        return -1;
    if (a_ptr->vaddr > b_ptr->vaddr)
        return 1;
#else
    if (a_ptr->paddr < b_ptr->paddr)
        return -1;
    if (a_ptr->paddr > b_ptr->paddr)
        return 1;
#endif
    return 0;
}

// Memory map address search function.
static int proc_memmap_search(void const *a, void const *_b) {
    proc_memmap_ent_t const *a_ptr = a;
    size_t                   b     = (size_t)_b;
#if !CONFIG_NOMMU
    if (a_ptr->vaddr < b)
        return -1;
    if (a_ptr->vaddr > b)
        return 1;
#else
    if (a_ptr->paddr < b)
        return -1;
    if (a_ptr->paddr > b)
        return 1;
#endif
    return 0;
}

#if !CONFIG_NOMMU
// Allocate more memory to a process.
// Returns actual virtual address on success, 0 on failure.
errno_size_t proc_map_raw(process_t *proc, size_t vaddr_req, size_t min_size, size_t min_align, uint32_t flags) {
    proc_memmap_t *map  = &proc->memmap;
    flags              |= VMM_FLAG_A | VMM_FLAG_D;

    // Correct virtual address.
    if (min_align & (min_align - 1)) {
        logkf(LOG_WARN, "min_align=%{size;d} ignored because it is not a power of 2", min_align);
        min_align = CONFIG_PAGE_SIZE;
    } else if (min_align < CONFIG_PAGE_SIZE) {
        min_align = CONFIG_PAGE_SIZE;
    }
    if (vaddr_req < 65536 || vaddr_req < CONFIG_PAGE_SIZE) {
        vaddr_req = 65536 > CONFIG_PAGE_SIZE ? 65536 : CONFIG_PAGE_SIZE;
    }
    if (vaddr_req & (min_align - 1)) {
        vaddr_req += min_align - (vaddr_req & (min_align - 1));
    }
    if (min_size & (min_align - 1)) {
        min_size += min_align - (min_size & (min_align - 1));
    }
    if (!mmu_is_canon_user_range(vaddr_req, min_size)) {
        vaddr_req = mmu_canon_half_size() - min_size;
        if (vaddr_req & (min_align - 1)) {
            vaddr_req -= vaddr_req & (min_align - 1);
        }
        if (vaddr_req < 65536 || vaddr_req < CONFIG_PAGE_SIZE) {
            logk(LOG_WARN, "Impossible vmem request");
            return -ENOMEM;
        }
    }

    // TODO: Disambiguate virtual addresses.
    uint32_t existing = proc_map_contains_raw(proc, vaddr_req, min_size);
    if (existing) {
        logk(LOG_WARN, "Overlapping virtual address requested");
        return -ENOMEM;
    }

    // Convert to page numbers.
    size_t vpn   = vaddr_req / CONFIG_PAGE_SIZE;
    size_t pages = min_size / CONFIG_PAGE_SIZE;
    size_t i     = 0;
    while (i < pages) {
        size_t alloc, ppn;
        for (alloc = (size_t)1 << __builtin_ctzll(((pages - i) | (vpn + i)) / CONFIG_PAGE_SIZE); alloc; alloc >>= 1) {
            ppn = phys_page_alloc(alloc, true);
            if (ppn) {
                break;
            }
        }
        if (!alloc) {
            goto nomem;
        }
        proc_memmap_ent_t new_ent = {
            .paddr = ppn * CONFIG_PAGE_SIZE,
            .vaddr = (vpn + i) * CONFIG_PAGE_SIZE,
            .size  = alloc * CONFIG_PAGE_SIZE,
            .write = true,
            .exec  = true,
        };
        if (!array_lencap_sorted_insert(
                &map->regions,
                sizeof(proc_memmap_ent_t),
                &map->regions_len,
                &map->regions_cap,
                &new_ent,
                proc_memmap_cmp
            )) {
            phys_page_free(ppn);
            goto nomem;
        }
        if (vmm_map_u_at(&proc->memmap.mem_ctx, vpn + i, alloc, ppn, flags)) {
            goto nomem;
        }
        logkf(LOG_INFO, "Mapped %{size;d} bytes at %{size;x} to process %{d}", new_ent.size, new_ent.vaddr, proc->pid);
        i += alloc;
    }

    return (errno_size_t)vaddr_req;
nomem:
    logk(LOG_WARN, "TODO: Cleanup when proc_map_raw partially fails");
    return -ENOMEM;
}

// Split the memory map until `vaddr` to `vaddr+len` is an unique range.
static inline size_t split_regions(proc_memmap_t *map, size_t index, size_t vaddr, size_t len) {
    TODO();
}

// Release memory allocated to a process from `vaddr` to `vaddr+len`.
// The given span should not fall outside an area mapped with `proc_map_raw`.
errno_t proc_unmap_raw(process_t *proc, size_t vaddr, size_t len) {
    logk(LOG_WARN, "TODO: proc_unmap_raw");
    return 0;

    proc_memmap_t *map = &proc->memmap;
    if (!(proc_map_contains_raw(proc, vaddr, len) & 8)) {
        return -EINVAL;
    }

    // Align `vaddr` and `len` to page sizes.
    if (vaddr % CONFIG_PAGE_SIZE) {
        len   += vaddr % CONFIG_PAGE_SIZE;
        vaddr -= vaddr % CONFIG_PAGE_SIZE;
    }
    if (len % CONFIG_PAGE_SIZE) {
        len += CONFIG_PAGE_SIZE - len % CONFIG_PAGE_SIZE;
    }

    // Array of pages to free.
    size_t *to_free     = NULL;
    size_t  to_free_len = 0, to_free_cap = 0;

    // Find start of mapped region.
    array_binsearch_t search =
        array_binsearch(map->regions, sizeof(proc_memmap_ent_t), map->regions_len, (void *)vaddr, proc_memmap_search);
    assert_dev_drop(search.found);
    size_t index = search.index;
    assert_dev_drop(map->regions[index].vaddr <= vaddr);
    assert_dev_drop(vaddr < map->regions[index].vaddr + map->regions[index].size);

    // Split mapped regions and unmap them from the process.
    while (len) {
        // Get sub-block offset; it dictates how to split and release memory.
        size_t page_off = vaddr - map->regions[index].vaddr;
        size_t order    = __builtin_ctzll((page_off | map->regions[index].size) / CONFIG_PAGE_SIZE);

        // Split the memory and remove the region in question.
        size_t            split_index = split_regions(map, index, vaddr, CONFIG_PAGE_SIZE << order);
        proc_memmap_ent_t removed;
        array_lencap_remove(
            &map->regions,
            sizeof(proc_memmap_ent_t),
            &map->regions_len,
            &map->regions_cap,
            &removed,
            split_index
        );

        // Add it to the list of blocks to free and remove it from the page table.
        size_t split_ppn = removed.paddr / CONFIG_PAGE_SIZE;
        if (!array_lencap_insert(&to_free, sizeof(size_t), &to_free_len, &to_free_cap, &split_ppn, to_free_len)) {
            logk(LOG_FATAL, "Out of memory");
            panic_abort();
        }
        // memprotect_u(&map->mpu_ctx, vaddr, 0, (size_t)CONFIG_PAGE_SIZE << order, 0);
        logkf(
            LOG_INFO,
            "Unmapped %{size;d} bytes at %{size;x} from process %{d}",
            (size_t)CONFIG_PAGE_SIZE << order,
            vaddr,
            proc->pid
        );

        // Remove it from the space to unmap.
        vaddr += (size_t)CONFIG_PAGE_SIZE << order;
        len   -= (size_t)CONFIG_PAGE_SIZE << order;
        index  = split_index;
    }

    // Release those same pages.
    for (size_t i = 0; i < to_free_len; i++) {
        phys_page_free(to_free[i]);
    }
    free(to_free);

    return 0;
}

// Whether the process owns this range of virtual memory.
// Returns the lowest common denominator of the access bits.
int proc_map_contains_raw(process_t *proc, size_t vaddr, size_t size) {
    if (!mmu_is_canon_user_range(vaddr, size)) {
        return 0;
    }
    int flags = VMM_FLAG_RWX;
    while (true) {
        virt2phys_t info = vmm_virt2phys(&proc->memmap.mem_ctx, vaddr);
        if (info.flags & VMM_FLAG_RWX) {
            flags &= (int)info.flags;
        } else {
            return 0;
        }
        if (!flags) {
            return 0;
        }
        size_t inc = info.size - (vaddr & (info.size - 1));
        if (inc >= size) {
            return flags;
        }
        size  -= inc;
        vaddr += inc;
    }
}

#else

// Allocate more memory to a process.
errno_size_t proc_map_raw(process_t *proc, size_t vaddr_req, size_t min_size, size_t min_align, uint32_t flags) {
    TODO();
}

// Release memory allocated to a process.
errno_t proc_unmap_raw(process_t *proc, size_t base) {
    TODO();
}

// Whether the process owns this range of virtual memory.
// Returns the lowest common denominator of the access bits.
int proc_map_contains_raw(process_t *proc, size_t vaddr, size_t size) {
    TODO();
}
#endif



// Allocate more memory to a process.
// Returns actual virtual address on success, 0 on failure.
errno_size_t proc_map(pid_t pid, size_t vaddr_req, size_t min_size, size_t min_align, int flags) {
    mutex_acquire(&proc_mtx, TIMESTAMP_US_MAX);
    process_t   *proc = proc_get(pid);
    errno_size_t res;
    if (proc) {
        res = proc_map_raw(proc, vaddr_req, min_size, min_align, flags);
    } else {
        res = -ENOENT;
    }
    mutex_release(&proc_mtx);
    return res;
}

// Release memory allocated to a process.
// The given span should not fall outside an area mapped with `proc_map_raw`.
errno_t proc_unmap(pid_t pid, size_t vaddr, size_t len) {
    mutex_acquire(&proc_mtx, TIMESTAMP_US_MAX);
    process_t *proc = proc_get(pid);
    errno_t    res;
    if (proc) {
        res = proc_unmap_raw(proc, vaddr, len);
    } else {
        res = -ENOENT;
    }
    mutex_release(&proc_mtx);
    return res;
}

// Whether the process owns this range of memory.
// Returns the lowest common denominator of the access bits bitwise or 8.
// Returns -errno on error.
errno_t proc_map_contains(pid_t pid, size_t base, size_t size) {
    mutex_acquire_shared(&proc_mtx, TIMESTAMP_US_MAX);
    process_t *proc = proc_get(pid);
    errno_t    res;
    if (proc) {
        res = proc_map_contains_raw(proc, base, size);
    } else {
        res = -ENOENT;
    }
    mutex_release_shared(&proc_mtx);
    return res;
}
