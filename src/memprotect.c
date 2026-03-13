
// SPDX-License-Identifier: MIT

#include "memprotect.h"

#include "arrays.h"
#include "assertions.h"
#include "badge_strings.h"
#include "cpu/interrupt.h"
#include "cpu/mmu.h"
#include "isr_ctx.h"
#include "page_alloc.h"
#include "panic.h"
#include "spinlock.h"

// Page table walk result.
typedef struct {
    // Physical address of last loaded PTA.
    size_t    paddr;
    // Last loaded PTE.
    mmu_pte_t pte;
    // Page table level of PTE.
    uint8_t   level;
    // Whether the subject page was found.
    bool      found;
    // Whether the virtual address is valid.
    bool      vaddr_valid;
} pt_walk_t;

// Allocable virtual memory area.
typedef struct {
    // Base virtual page number.
    size_t vpn;
    // Size in pages.
    size_t pages;
} vmm_info_t;

// Sort `vmm_info_t` by `vpn`.
static int vmm_info_cmp(void const *a, void const *b) {
    vmm_info_t const *info_a = a;
    vmm_info_t const *info_b = b;
    if (info_a->vpn > info_b->vpn) {
        return 1;
    } else if (info_a->vpn < info_b->vpn) {
        return -1;
    } else {
        return 0;
    }
}

// Allocated VMM ranges that can be freed.
static size_t      vmm_used_len, vmm_used_cap;
// Allocated VMM ranges that can be freed.
static vmm_info_t *vmm_used;
// Unallocated VMM ranges.
static size_t      vmm_free_len, vmm_free_cap;
// Unallocated VMM ranges.
static vmm_info_t *vmm_free;



// At least one naturally-aligned page of zeroes.
uint8_t const *memprotect_zeroes;
// At least one naturally-aligned page of zeroes.
size_t         memprotect_zeroes_len;

// At least one naturally-aligned page of zeroes.
size_t memprotect_zeroes_paddr;

// HHDM length in pages.
size_t memprotect_hhdm_pages;
// Kernel virtual page number.
size_t memprotect_kernel_vpn;
// Kernel physical page number.
size_t memprotect_kernel_ppn;
// Kernel length in pages.
size_t memprotect_kernel_pages;

/* ==== Kernel sections ==== */
// NOLINTBEGIN
extern char const __start_text[];
extern char const __stop_text[];
extern char const __start_rodata[];
extern char const __stop_rodata[];
extern char const __start_data[];
extern char const __stop_data[];
// NOLINTEND

// For systems with VMEM: global MMU context.
mpu_ctx_t      mpu_global_ctx;
// All active non-global MMU contexts.
static dlist_t ctx_list = DLIST_EMPTY;



// Walk a page table; vaddr to paddr or return where the page table ends.
static pt_walk_t pt_walk(size_t pt_ppn, size_t vpn) {
    if (vpn >= mmu_half_pages && vpn < mmu_high_vpn) {
        return (pt_walk_t){0};
    }

    // Place the next VPN at the MSB for easy extraction.
    size_t    pte_addr = 0;
    mmu_pte_t pte      = {0};

    // Walk the page table.
    for (int i = 0; i < mmu_levels; i++) {
        int level  = mmu_levels - i - 1;
        pte_addr   = pt_ppn * CONFIG_PAGE_SIZE + mmu_vpn_part(vpn, level) * sizeof(mmu_pte_t);
        pte        = mmu_read_pte(pte_addr);
        size_t ppn = mmu_pte_get_ppn(pte, level);

        if (!mmu_pte_is_valid(pte, level)) {
            // PTE invalid.
            return (pt_walk_t){pte_addr, pte, level, false, true};
        } else if (mmu_pte_is_leaf(pte, level)) {
// Leaf PTE.
#if MMU_SUPPORT_SUPERPAGES
            if (level && ppn & ((1 << MMU_BITS_PER_LEVEL * level) - 1)) {
                // Misaligned superpage.
                logkf(LOG_FATAL, "PT corrupt: L%{d} leaf PTE @ %{size;x} (%{size;x}) misaligned", i, pte_addr, pte.val);
                logkf(LOG_FATAL, "Offending VADDR: %{size;x}", vpn * CONFIG_PAGE_SIZE);
                panic_abort();
            }
#else
            if (level) {
                // MMU does not support superpages.
                logkf(
                    LOG_FATAL,
                    "PT corrupt: L%{d} leaf PTE @ %{size;x} (%{size;x}) but MMU does not support superpages",
                    i,
                    pte_addr,
                    pte.val
                );
                logkf(LOG_FATAL, "Offending VADDR: %{size;x}", vpn * CONFIG_PAGE_SIZE);
                panic_abort();
            }
#endif
            // Valid leaf PTE.
            return (pt_walk_t){pte_addr, pte, level, true, true};
        } else if (level) {
            // Non-leaf PTE.
            pt_ppn = ppn;
        }
    }

    // Not a leaf node.
    logkf(LOG_FATAL, "PT corrupt: L0 PTE @ %{size;x} (%{size;x}) not a leaf PTE", pte_addr, pte.val);
    logkf(LOG_FATAL, "Offending VADDR: %{size;x}", vpn * CONFIG_PAGE_SIZE);
    panic_abort();
}

// Split a PTE into the next page table level to allow for edits of superpages.
// Returns PPN of new table.
static inline size_t pt_split(size_t pt_ppn, int pt_level, size_t pte_paddr, mmu_pte_t pte, size_t vpn) {
#if MMU_SUPPORT_SUPERPAGES
    (void)pt_ppn;
    (void)pt_level;
    (void)vpn;

    // Allocate new table.
    size_t next_pt = phys_page_alloc(1, false);
    assert_always(next_pt);

    // Populate with new entries.
    size_t   ppn   = mmu_pte_get_ppn(pte, pt_level);
    uint32_t flags = mmu_pte_get_flags(pte, pt_level);
    for (size_t i = 0; i < (1 << MMU_BITS_PER_LEVEL); i++) {
        size_t low_pte_paddr = next_pt * CONFIG_PAGE_SIZE + i * sizeof(mmu_pte_t);
        mmu_write_pte(low_pte_paddr, mmu_pte_new_leaf(ppn + i, pt_level - 1, flags));
    }

    // Overwrite old entry.
    mmu_write_pte(pte_paddr, mmu_pte_new(next_pt, pt_level));
    return next_pt;
#else
    (void)pt_ppn;

    // MMU does not support superpages.
    logkf(
        LOG_FATAL,
        "PT corrupt: L%{d} leaf PTE @ %{size;x} (%{size;x}) but MMU does not support superpages",
        pt_level,
        pte_paddr,
        pte.val
    );
    logkf(LOG_FATAL, "Offending VADDR: %{size;x}", vpn * CONFIG_PAGE_SIZE);
    panic_abort();
#endif
}

// Walk from an arbitrary page table level to another to create a new mapping.
// Returns true if the top level of the page table was edited.
static bool pt_map_1(size_t pt_ppn, int pt_level, size_t vpn, size_t ppn, int pte_level, uint32_t flags) {
    bool top_edit      = pt_level == pte_level;
    int  orig_pt_level = pt_level;

    // Validate paramters.
    assert_dev_drop(pt_level >= 0 && pt_level < mmu_levels);
#if MMU_SUPPORT_SUPERPAGES
    assert_dev_drop(pte_level >= 0 && pte_level < mmu_levels);
#else
    assert_dev_drop(pte_level == 0);
    pte_level = 0;
#endif
    assert_dev_drop(flags & MEMPROTECT_FLAG_RWX);
    assert_dev_drop((flags & MEMPROTECT_FLAG_RW) != MEMPROTECT_FLAG_W);
    assert_dev_drop(pte_level <= pt_level);

    // Walk the page table, adding new tables as needed.
    for (; pte_level < pt_level; pt_level--) {
        size_t    pte_paddr = pt_ppn * CONFIG_PAGE_SIZE + mmu_vpn_part(vpn, pt_level) * sizeof(mmu_pte_t);
        // Too high; read PTE to go to next table.
        mmu_pte_t pte       = mmu_read_pte(pte_paddr);
        if (!mmu_pte_is_valid(pte, pt_level)) {
            // Make new level of page table.
            size_t next_pt = phys_page_alloc(1, false);
            assert_always(next_pt);
            mmu_write_pte(pte_paddr, mmu_pte_new(next_pt, pt_level));
            top_edit |= orig_pt_level == pt_level;
            pt_ppn    = next_pt;
        } else if (mmu_pte_is_leaf(pte, pt_level)) {
            // Split page.
            pt_ppn    = pt_split(pt_ppn, pt_level, pte_paddr, pte, vpn);
            top_edit |= orig_pt_level == pt_level;
        } else {
            // Walk to next level of page table.
            pt_ppn = mmu_pte_get_ppn(pte, pt_level);
        }
    }

    // At the correct level; write leaf PTE.
    size_t pte_paddr = pt_ppn * CONFIG_PAGE_SIZE + mmu_vpn_part(vpn, pte_level) * sizeof(mmu_pte_t);
    mmu_write_pte(pte_paddr, mmu_pte_new_leaf(ppn, pte_level, flags));
    return top_edit;
}

// Walk from an arbitrary page table level to another to remove a mapping.
// Returns true if the top level of the page table was edited.
static bool pt_unmap_1(size_t pt_ppn, int pt_level, size_t vpn, int pte_level) {
    bool top_edit      = pt_level == pte_level;
    int  orig_pt_level = pt_level;

    // Validate paramters.
    assert_dev_drop(pt_level >= 0 && pt_level < mmu_levels);
#if MMU_SUPPORT_SUPERPAGES
    assert_dev_drop(pte_level >= 0 && pte_level < mmu_levels);
#else
    assert_dev_drop(pte_level == 0);
    pte_level = 0;
#endif
    assert_dev_drop(pte_level <= pt_level);

    // Walk the page table, adding new tables as needed.
    for (; pte_level < pt_level; pt_level--) {
        size_t    pte_paddr = pt_ppn * CONFIG_PAGE_SIZE + mmu_vpn_part(vpn, pt_level) * sizeof(mmu_pte_t);
        // Too high; read PTE to go to next table.
        mmu_pte_t pte       = mmu_read_pte(pte_paddr);
        if (!mmu_pte_is_valid(pte, pt_level)) {
            // Nothing mapped here.
            return top_edit;
        } else if (mmu_pte_is_leaf(pte, pt_level)) {
            // Split page.
            pt_ppn    = pt_split(pt_ppn, pt_level, pte_paddr, pte, vpn);
            top_edit |= orig_pt_level == pt_level;
        } else {
            // Walk to next level of page table.
            pt_ppn = mmu_pte_get_ppn(pte, pt_level);
        }
    }

    // At the correct level; write leaf PTE.
    size_t pte_paddr = pt_ppn * CONFIG_PAGE_SIZE + mmu_vpn_part(vpn, pte_level) * sizeof(mmu_pte_t);
    mmu_write_pte(pte_paddr, MMU_PTE_NULL);
    return top_edit;
}

// Calculate the biggest superpage level that will fit a range.
static int pt_calc_superpage(int max_level, size_t vpn, size_t ppn, size_t max_len) {
#if MMU_SUPPORT_SUPERPAGES
#ifdef MMU_LEAF_MAX_LEVEL
    if (max_level > MMU_LEAF_MAX_LEVEL) {
        max_level = MMU_LEAF_MAX_LEVEL;
    }
#endif
    for (int i = max_level; i > 0; i--) {
        size_t super_len = 1LLU << (MMU_BITS_PER_LEVEL * i);
        if ((vpn & (super_len - 1)) == 0 && (ppn & (super_len - 1)) == 0 && max_len >= super_len) {
            return i;
        }
    }
#endif
    return 0;
}

// Walk from an arbitrary page table level to create new mappings.
// Returns true if the top level of the page table was edited.
static bool pt_map(size_t pt_ppn, int pt_level, size_t vpn, size_t ppn, size_t pages, uint32_t flags) {
    bool top_edit = false;
    while (pages) {
        int pte_level     = pt_calc_superpage(pt_level, vpn, ppn, pages);
        top_edit         |= pt_map_1(pt_ppn, pt_level, vpn, ppn, pte_level, flags);
        size_t super_len  = 1LLU << (MMU_BITS_PER_LEVEL * pte_level);
        vpn              += super_len;
        ppn              += super_len;
        pages            -= super_len;
    }
    return top_edit;
}

// Walk from an arbitrary page table level to remove mappings.
// Returns true if the top level of the page table was edited.
static bool pt_unmap(size_t pt_ppn, int pt_level, size_t vpn, size_t pages) {
    bool top_edit = false;
    while (pages) {
        int pte_level     = pt_calc_superpage(pt_level, vpn, 0, pages);
        top_edit         |= pt_unmap_1(pt_ppn, pt_level, vpn, pte_level);
        size_t super_len  = 1LLU << (MMU_BITS_PER_LEVEL * pte_level);
        vpn              += super_len;
        pages            -= super_len;
    }
    return top_edit;
}

// Broadcast global mappings.
static void broadcast_to(size_t dest_pt_ppn, size_t src_pt_ppn) {
    for (size_t i = 1 << (MMU_BITS_PER_LEVEL - 1); i < (1 << MMU_BITS_PER_LEVEL); i++) {
        mmu_pte_t pte = mmu_read_pte(src_pt_ppn * CONFIG_PAGE_SIZE + i * sizeof(mmu_pte_t));
        if (mmu_pte_is_valid(pte, mmu_levels - 1)) {
            mmu_write_pte(dest_pt_ppn * CONFIG_PAGE_SIZE + i * sizeof(mmu_pte_t), pte);
        }
    }
}



// Lookup virtual address to physical address.
virt2phys_t memprotect_virt2phys(mpu_ctx_t *ctx, size_t vaddr) {
    if (!ctx) {
        ctx = &mpu_global_ctx;
    }
    if (vaddr >= mmu_half_size && vaddr < mmu_high_vaddr) {
        return (virt2phys_t){0};
    }
    pt_walk_t walk = pt_walk(ctx->root_ppn, vaddr / CONFIG_PAGE_SIZE);
    if (walk.found) {
        size_t page_size  = CONFIG_PAGE_SIZE << (MMU_BITS_PER_LEVEL * walk.level);
        size_t page_paddr = mmu_pte_get_ppn(walk.pte, walk.level) * CONFIG_PAGE_SIZE;
        return (virt2phys_t){
            mmu_pte_get_flags(walk.pte, walk.level),
            page_paddr + (vaddr & (page_size - 1)),
            vaddr & ~(page_size - 1),
            page_paddr,
            page_size,
        };
    }
    return (virt2phys_t){0};
}

// Mark a range of VMM as reserved.
static void vmm_mark_reserved(size_t vpn, size_t pages) {
    // Clamp address range.
    if (vpn < mmu_high_vpn) {
        pages -= mmu_high_vpn - vpn;
        vpn    = mmu_high_vpn;
    }
    if (vpn + pages > mmu_high_vpn + mmu_half_size) {
        pages = mmu_high_vpn + mmu_half_size - vpn;
    }

    // Look up which entry to cut.
    array_binsearch_t idx = array_binsearch(vmm_free, sizeof(vmm_info_t), vmm_free_len, &vpn, vmm_info_cmp);
    assert_dev_drop(idx.found || idx.index > 0);
    idx.index        -= !idx.found;
    vmm_info_t range  = vmm_free[idx.index];
    assert_dev_drop(vpn >= range.vpn);
    assert_dev_drop(vpn + pages <= range.vpn + range.pages);

    if (range.vpn == vpn && range.pages == pages) {
        // Remove the entry from free list.
        array_lencap_remove(&vmm_free, sizeof(vmm_info_t), &vmm_free_len, &vmm_free_cap, NULL, idx.index);

    } else if (vpn > range.vpn && vpn + pages < range.vpn + range.pages) {
        // Add entry to the free list.
        vmm_info_t new_ent = {
            .vpn   = vpn + pages,
            .pages = range.vpn + range.pages - vpn - pages,
        };
        assert_always(
            array_lencap_insert(&vmm_free, sizeof(vmm_info_t), &vmm_free_len, &vmm_free_cap, &new_ent, idx.index + 1)
        );
        // Modify the other one.
        vmm_free[idx.index] = (vmm_info_t){
            .vpn   = range.vpn,
            .pages = vpn - range.vpn,
        };

    } else if (vpn > range.vpn) {
        // Modify the entry.
        vmm_free[idx.index].pages = vpn - range.vpn;

    } else /* vpn + pages < range.vpn + range.pages */ {
        // Modify the entry.
        vmm_free[idx.index].vpn   = vpn + pages;
        vmm_free[idx.index].pages = range.vpn + range.pages - vpn - pages;
    }
}

// Alloc vaddr lock.
static spinlock_t vmm_lock = SPINLOCK_T_INIT;


// Allocare a kernel virtual address to a certain physical address.
size_t memprotect_alloc_vaddr(size_t len) {
    bool ie = irq_disable();
    spinlock_take(&vmm_lock);
    size_t pages = (len - 1) / CONFIG_PAGE_SIZE + 3;
    size_t i;
    for (i = 0; i < vmm_free_len; i++) {
        if (vmm_free[i].pages >= pages) {
            break;
        }
    }
    assert_always(i < vmm_free_len);
    vmm_info_t range      = vmm_free[i];
    vmm_info_t used_range = {
        .vpn   = range.vpn,
        .pages = pages,
    };
    assert_always(array_lencap_sorted_insert(
        &vmm_used,
        sizeof(vmm_info_t),
        &vmm_used_len,
        &vmm_used_cap,
        &used_range,
        vmm_info_cmp
    ));
    if (vmm_free[i].pages > pages) {
        vmm_free[i].vpn   += pages;
        vmm_free[i].pages -= pages;
    } else {
        array_lencap_remove(&vmm_free, sizeof(vmm_info_t), &vmm_free_len, &vmm_free_cap, NULL, i);
    }
    spinlock_release(&vmm_lock);
    irq_enable_if(ie);
    return (range.vpn + 1) * CONFIG_PAGE_SIZE;
}

// Free a virtual address range allocated with `memprotect_alloc_vaddr`.
void memprotect_free_vaddr(size_t vaddr) {
    assert_always(vaddr % CONFIG_PAGE_SIZE == 0);
    size_t vpn = vaddr / CONFIG_PAGE_SIZE - 1;
    bool   ie  = irq_disable();
    spinlock_take(&vmm_lock);

    // Look up the in-use entry.
    array_binsearch_t res = array_binsearch(vmm_used, sizeof(vmm_info_t), vmm_used_len, &vpn, vmm_info_cmp);
    assert_always(res.found);
    vmm_info_t range;
    array_lencap_remove(&vmm_used, sizeof(vmm_info_t), &vmm_used_len, &vmm_used_cap, &range, res.index);
    assert_always(range.vpn == vpn);

    // Insert into the free list.
    res = array_binsearch(vmm_free, sizeof(vmm_info_t), vmm_free_len, &range, vmm_info_cmp);
    assert_dev_drop(!res.found);
    if (res.index && vmm_free[res.index - 1].vpn + vmm_free[res.index - 1].pages == range.vpn &&
        range.vpn + range.pages == vmm_free[res.index].vpn) {
        // Merge both.
        vmm_free[res.index - 1].pages =
            vmm_free[res.index].vpn + vmm_free[res.index].pages - vmm_free[res.index - 1].vpn;
        array_lencap_remove(&vmm_free, sizeof(vmm_info_t), &vmm_free_len, &vmm_free_cap, NULL, res.index);

    } else if (res.index && vmm_free[res.index - 1].vpn + vmm_free[res.index - 1].pages == range.vpn) {
        // Merge left.
        vmm_free[res.index - 1].pages = range.vpn + range.pages - vmm_free[res.index - 1].vpn;

    } else if (range.vpn + range.pages == vmm_free[res.index].vpn) {
        // Merge right.
        vmm_free[res.index].pages += vmm_free[res.index].vpn - range.vpn;
        vmm_free[res.index].vpn    = range.vpn;

    } else {
        // Not mergable.
        assert_always(
            array_lencap_insert(&vmm_free, sizeof(vmm_info_t), &vmm_free_len, &vmm_free_cap, &range, res.index)
        );
    }

    spinlock_release(&vmm_lock);
    irq_enable_if(ie);
}


// Initialise memory protection driver.
void memprotect_early_init() {
    // Initialize MMU driver.
    mmu_early_init();
}

// Initialise memory protection driver.
void memprotect_postheap_init() {
    // Mark entire space as free.
    vmm_info_t mem;
    mem.vpn      = mmu_high_vpn;
    mem.pages    = mmu_half_pages;
    vmm_free     = malloc(sizeof(vmm_info_t));
    vmm_free[0]  = mem;
    vmm_free_len = 1;
    vmm_free_cap = 1;

    // Remove HHDM and kernel from free area.
    vmm_mark_reserved(memprotect_kernel_vpn - 1, memprotect_kernel_pages + 2);
    vmm_mark_reserved(mmu_hhdm_vpn - 1, memprotect_hhdm_pages + 2);

    // Allocate global page table.
    mpu_global_ctx.root_ppn = phys_page_alloc(1, false);
    assert_always(mpu_global_ctx.root_ppn);

    // HHDM mapping (without execute this time).
    pt_map(
        mpu_global_ctx.root_ppn,
        mmu_levels - 1,
        mmu_hhdm_vaddr / CONFIG_PAGE_SIZE,
        0,
        memprotect_hhdm_pages,
        MEMPROTECT_FLAG_RW | MEMPROTECT_FLAG_GLOBAL | MEMPROTECT_FLAG_KERNEL
    );
    // Create kernel mappings.
    size_t vpn = memprotect_kernel_vpn;
    size_t ppn = memprotect_kernel_ppn;
    size_t sect_len;
    // Kernel RX.
    sect_len = (__stop_text - __start_text) / CONFIG_PAGE_SIZE;
    assert_dev_drop((__stop_text - __start_text) % CONFIG_PAGE_SIZE == 0);
    pt_map(
        mpu_global_ctx.root_ppn,
        mmu_levels - 1,
        vpn,
        ppn,
        sect_len,
        MEMPROTECT_FLAG_RX | MEMPROTECT_FLAG_GLOBAL | MEMPROTECT_FLAG_KERNEL
    );
    vpn      += sect_len;
    ppn      += sect_len;
    // Kernel R.
    sect_len  = (__stop_rodata - __start_rodata) / CONFIG_PAGE_SIZE;
    assert_dev_drop((__stop_rodata - __start_rodata) % CONFIG_PAGE_SIZE == 0);
    pt_map(
        mpu_global_ctx.root_ppn,
        mmu_levels - 1,
        vpn,
        ppn,
        sect_len,
        MEMPROTECT_FLAG_R | MEMPROTECT_FLAG_GLOBAL | MEMPROTECT_FLAG_KERNEL
    );
    vpn      += sect_len;
    ppn      += sect_len;
    // Kernel RW.
    sect_len  = (__stop_data - __start_data) / CONFIG_PAGE_SIZE;
    assert_dev_drop((__stop_data - __start_data) % CONFIG_PAGE_SIZE == 0);
    pt_map(
        mpu_global_ctx.root_ppn,
        mmu_levels - 1,
        vpn,
        ppn,
        sect_len,
        MEMPROTECT_FLAG_RW | MEMPROTECT_FLAG_GLOBAL | MEMPROTECT_FLAG_KERNEL
    );
    // Switch over to new page table.
    atomic_thread_fence(memory_order_release);
    memprotect_swap_from_isr();
    logkf_from_isr(LOG_INFO, "Virtual memory initialized, %{d} paging levels", mmu_levels);

    // Run other MMU init code.
    mmu_init();

    // Page-or-more of zeroes.
    memprotect_zeroes_paddr = phys_page_alloc(1, false) * CONFIG_PAGE_SIZE;
    memprotect_zeroes_len   = CONFIG_PAGE_SIZE;
    size_t vaddr            = memprotect_alloc_vaddr(CONFIG_PAGE_SIZE);
    memprotect_k(vaddr, memprotect_zeroes_paddr, memprotect_zeroes_len, MEMPROTECT_FLAG_R);
    memprotect_commit(&mpu_global_ctx);
    memprotect_zeroes = (void *)vaddr;
}

// Initialise memory protection driver.
void memprotect_init() {
}



// Create a memory protection context.
void memprotect_create(mpu_ctx_t *ctx) {
    ctx->node     = DLIST_NODE_EMPTY;
    ctx->root_ppn = phys_page_alloc(1, false);
    assert_always(ctx->root_ppn);
    size_t src_vaddr  = mmu_hhdm_vaddr + mpu_global_ctx.root_ppn * CONFIG_PAGE_SIZE;
    size_t dest_vaddr = mmu_hhdm_vaddr + ctx->root_ppn * CONFIG_PAGE_SIZE;
    mem_copy((void *)dest_vaddr, (void const *)src_vaddr, CONFIG_PAGE_SIZE);
    dlist_append(&ctx_list, &ctx->node);
}

// Clean up a memory protection context.
void memprotect_destroy(mpu_ctx_t *ctx) {
    dlist_remove(&ctx_list, &ctx->node);
    logkf(LOG_DEBUG, "TODO: Memprotect cleanup");
}

// Add a memory protection region.
void memprotect_impl(mpu_ctx_t *ctx, size_t vpn, size_t ppn, size_t pages, uint32_t flags) {
    bool top_mod = false;

    // Add R if W is set.
    if (flags & MEMPROTECT_FLAG_W) {
        flags |= MEMPROTECT_FLAG_R;
    }

    // Read current value here.
    pt_walk_t walk         = pt_walk(ctx->root_ppn, vpn);
    bool      needs_ifence = false;
    if ((mmu_pte_get_flags(walk.pte, walk.level) & MEMPROTECT_FLAG_X) != (flags & MEMPROTECT_FLAG_X)) {
        needs_ifence = true;
    }
    ctx->needs_ifence |= needs_ifence;

    // Apply change.
    if (flags & MEMPROTECT_FLAG_RWX) {
        top_mod = pt_map(ctx->root_ppn, mmu_levels - 1, vpn, ppn, pages, flags);
    } else {
        top_mod = pt_unmap(ctx->root_ppn, mmu_levels - 1, vpn, pages);
    }

    // Broadcast global changes.
    if (top_mod && (flags & MEMPROTECT_FLAG_GLOBAL)) {
        dlist_node_t *node = ctx_list.head;
        while (node) {
            broadcast_to(mpu_global_ctx.root_ppn, ((mpu_ctx_t *)node)->root_ppn);
            node = node->next;
        }
    }
}

// Add a memory protection region for user memory.
bool memprotect_u(mpu_ctx_t *ctx, size_t vaddr, size_t paddr, size_t length, uint32_t flags) {
    if ((vaddr | paddr | length) % CONFIG_PAGE_SIZE) {
        // Misaligned.
        return false;
    }
    vaddr  /= CONFIG_PAGE_SIZE;
    paddr  /= CONFIG_PAGE_SIZE;
    length /= CONFIG_PAGE_SIZE;
    if (vaddr + length > mmu_half_pages) {
        // Out of bounds.
        return false;
    }
    flags &= ~(MEMPROTECT_FLAG_GLOBAL | MEMPROTECT_FLAG_KERNEL);
    memprotect_impl(ctx, vaddr, paddr, length, flags);
    return true;
}

// Add a memory protection region for kernel memory.
bool memprotect_k(size_t vaddr, size_t paddr, size_t length, uint32_t flags) {
    if ((vaddr | paddr | length) % CONFIG_PAGE_SIZE) {
        // Misaligned.
        return false;
    }
    vaddr  /= CONFIG_PAGE_SIZE;
    paddr  /= CONFIG_PAGE_SIZE;
    length /= CONFIG_PAGE_SIZE;
    if (vaddr < mmu_high_vpn || vaddr + length > mmu_high_vpn + mmu_half_pages) {
        // Out of bounds.
        return false;
    }
    flags |= MEMPROTECT_FLAG_GLOBAL | MEMPROTECT_FLAG_KERNEL;
    memprotect_impl(&mpu_global_ctx, vaddr, paddr, length, flags);
    return true;
}

// Commit pending memory protections, if any.
void memprotect_commit(mpu_ctx_t *ctx) {
    // TODO: Run mmu_vmem_fence and ifence on other CPUs.
    if (ctx->needs_ifence) {
        ctx->needs_ifence = false;
#if __riscv
        asm("fence.i");
#endif
    }
    mmu_vmem_fence();
    // TODO: Garbage collection is now safe to do.
}
