
// SPDX-License-Identifier: MIT

#include "arrays.h"
#include "assertions.h"
#include "bootp.h"
#include "device/class/pcictl.h"
#include "device/dev_class.h"
#include "device/device.h"
#include "ktest.h"
#include "log.h"
#include "mem/pmm.h"
#include "mem/vmm.h"
#include "panic.h"
#include "rawprint.h"
#include "set.h"

#ifdef __riscv
#include "device/dtb/dtparse.h"
#elif defined(__x86_64__)
#include "cpu/x86_ioport.h"
#include "uacpi/uacpi.h"
#endif

#include <stdbool.h>

#include <limine.h>

void init_pool(void *mem_start, void *mem_end, uint32_t flags);



#define REQ __attribute__((section(".requests")))

__attribute__((section(".requests_start"))) LIMINE_REQUESTS_START_MARKER;

LIMINE_BASE_REVISION(3);

REQ struct limine_memmap_request bootp_mm_req = {
    .id       = LIMINE_MEMMAP_REQUEST,
    .revision = 3,
};

#ifdef __riscv
REQ struct limine_dtb_request bootp_dtb_req = {
    .id       = LIMINE_DTB_REQUEST,
    .revision = 3,
};
#elif defined(__x86_64__)
REQ struct limine_rsdp_request bootp_rsdp_req = {
    .id       = LIMINE_RSDP_REQUEST,
    .revision = 3,
};
#endif

REQ struct limine_kernel_address_request bootp_addr_req = {
    .id       = LIMINE_KERNEL_ADDRESS_REQUEST,
    .revision = 3,
};

REQ struct limine_hhdm_request bootp_hhdm_req = {
    .id       = LIMINE_HHDM_REQUEST,
    .revision = 3,
};

REQ struct limine_kernel_file_request bootp_kernel_file_req = {
    .id       = LIMINE_KERNEL_FILE_REQUEST,
    .revision = 3,
};

REQ struct limine_executable_cmdline_request bootp_cmdline_req = {
    .id       = LIMINE_EXECUTABLE_CMDLINE_REQUEST,
    .revision = 3,
};

__attribute__((section(".requests_end"))) LIMINE_REQUESTS_END_MARKER;

#ifdef __x86_64__
// Returns the PHYSICAL address of the RSDP structure via *out_rsdp_address.
uacpi_status uacpi_kernel_get_rsdp(uacpi_phys_addr *out_rsdp_address) {
    if (bootp_rsdp_req.response) {
        *out_rsdp_address = (uacpi_phys_addr)bootp_rsdp_req.response->address - (uacpi_phys_addr)vmm_hhdm_vaddr;
        return UACPI_STATUS_OK;
    } else {
        logk(LOG_WARN, "uACPI asked for RSDP, but it does not exist");
        return UACPI_STATUS_NOT_FOUND;
    }
}
#endif



// Early hardware initialization.
void bootp_early_init() {
    rawprint("\033[0m\033[2J");

    // Verify needed requests have been answered.
    if (!bootp_mm_req.response) {
        logk_from_isr(LOG_FATAL, "Limine memmap response missing");
        panic_poweroff();
    }
#ifdef __riscv
    if (!bootp_dtb_req.response) {
        logk_from_isr(LOG_FATAL, "Limine DTB response missing");
        panic_poweroff();
    }
#elif defined(__x86_64__)
    if (!bootp_rsdp_req.response) {
        logk_from_isr(LOG_FATAL, "Limine RSDP response missing");
        panic_poweroff();
    }
#endif
    if (!bootp_addr_req.response) {
        logk_from_isr(LOG_FATAL, "Limine kernel address response missing");
        panic_poweroff();
    }
    if (!bootp_hhdm_req.response) {
        logk_from_isr(LOG_FATAL, "Limine HHDM response missing");
        panic_poweroff();
    }

    // Print memory map.
    struct limine_memmap_response *mem     = bootp_mm_req.response;
    char const *const              types[] = {
        [LIMINE_MEMMAP_USABLE]                 = "Usable",
        [LIMINE_MEMMAP_RESERVED]               = "Reserved",
        [LIMINE_MEMMAP_ACPI_RECLAIMABLE]       = "ACPI Reclaimable",
        [LIMINE_MEMMAP_ACPI_NVS]               = "ACPI NVS",
        [LIMINE_MEMMAP_BAD_MEMORY]             = "Bad",
        [LIMINE_MEMMAP_BOOTLOADER_RECLAIMABLE] = "Bootloader reclaimable",
        [LIMINE_MEMMAP_KERNEL_AND_MODULES]     = "Kernel",
        [LIMINE_MEMMAP_FRAMEBUFFER]            = "Framebuffer",
    };
    logk_from_isr(LOG_INFO, "Memory map:");
    size_t biggest_pool_size  = 0;
    size_t biggest_pool_index = 0;
    size_t usable_len         = 0;
    size_t reclaim_len        = 0;
    size_t usable_end         = 0;
    size_t usable_start       = SIZE_MAX;
    for (uint64_t i = 0; i < mem->entry_count; i++) {
        struct limine_memmap_entry *entry = mem->entries[i];
        logkf_from_isr(
            LOG_INFO,
            "%{u64;x}-%{u64;x} %{cs}",
            entry->base,
            entry->base + entry->length - 1,
            types[entry->type]
        );
        if (entry->type == LIMINE_MEMMAP_USABLE) {
            usable_len += entry->length;
        } else if (entry->type == LIMINE_MEMMAP_BOOTLOADER_RECLAIMABLE ||
                   entry->type == LIMINE_MEMMAP_ACPI_RECLAIMABLE) {
            reclaim_len += entry->length;
        }
#ifdef __riscv
        // Workaround: Limine has a bug where the DTB is in usable, not reclaimable, memory.
        if ((size_t)bootp_dtb_req.response->dtb_ptr - bootp_hhdm_req.response->offset < entry->base ||
            (size_t)bootp_dtb_req.response->dtb_ptr - bootp_hhdm_req.response->offset >= entry->base + entry->length) {
#endif
            // This second check was always there and just picks the largest candidate.
            if (entry->type == LIMINE_MEMMAP_USABLE && entry->length > biggest_pool_size) {
                biggest_pool_index = i;
                biggest_pool_size  = entry->length;
            }
#ifdef __riscv
        }
#endif
        if (entry->type != LIMINE_MEMMAP_BAD_MEMORY && entry->type != LIMINE_MEMMAP_RESERVED &&
            usable_start > entry->base) {
            usable_start = entry->base;
        }
        if (entry->type != LIMINE_MEMMAP_BAD_MEMORY && entry->type != LIMINE_MEMMAP_RESERVED &&
            usable_end < entry->base + entry->length) {
            usable_end = entry->base + entry->length;
        }
    }

    // Pass info to memory protection.
    if (bootp_addr_req.response->physical_base % CONFIG_PAGE_SIZE) {
        logkf_from_isr(LOG_FATAL, "Kernel is not aligned to page size");
        panic_poweroff();
    }
    vmm_hhdm_size    = usable_end - usable_start;
    vmm_hhdm_offset  = bootp_hhdm_req.response->offset;
    vmm_hhdm_vaddr   = vmm_hhdm_offset + usable_start;
    vmm_kernel_paddr = bootp_addr_req.response->physical_base;
    vmm_kernel_vaddr = bootp_addr_req.response->virtual_base;

    // Report memory stats.
    logkf_from_isr(
        LOG_INFO,
        "%{size;d} MiB total memory, %{size;d} MiB of which reclaimable",
        (usable_len + reclaim_len) >> 20,
        reclaim_len >> 20
    );

    // Initialize PMM with the biggest contiguous usable region now.
    struct limine_memmap_entry *early_pool = mem->entries[biggest_pool_index];
    logkf_from_isr(
        LOG_INFO,
        "Early pool at 0x%{size;x}-0x%{size;x}",
        early_pool->base,
        early_pool->base + early_pool->length - 1
    );
    pmm_init(
        (usable_start + CONFIG_PAGE_SIZE - 1) / CONFIG_PAGE_SIZE,
        usable_end / CONFIG_PAGE_SIZE,
        (early_pool->base + CONFIG_PAGE_SIZE - 1) / CONFIG_PAGE_SIZE,
        (early_pool->base + early_pool->length) / CONFIG_PAGE_SIZE
    );

    // Add the smaller contiguous usable regions to PMM.
    for (size_t i = 0; i < mem->entry_count; i++) {
        struct limine_memmap_entry const *ent = mem->entries[i];

        ppn_t start = (ent->base + CONFIG_PAGE_SIZE - 1) / CONFIG_PAGE_SIZE;
        ppn_t end   = (ent->base + ent->length) / CONFIG_PAGE_SIZE;

        if (ent->type == LIMINE_MEMMAP_USABLE && i != biggest_pool_index) {
            pmm_mark_free(start, end);
        } else if (ent->type == LIMINE_MEMMAP_KERNEL_AND_MODULES) {
            for (ppn_t ppn = start; ppn < end; ppn++) {
                pmm_page_struct(ppn)->flags = PAGE_USAGE_KERNEL_SEGMENT << PGFLAGS_USAGE_EXP;
            }
        }
    }

    ktests_runlevel(KTEST_WHEN_PMM);
}

// Post-heap protocol-dependent initialization; after the boot announcement log.
void bootp_postheap_init() {
    // Call Rust function that loads kernel parameters.
    void bootp_limine_load_kparams();
    bootp_limine_load_kparams();
}

// Full hardware initialization.
void bootp_full_init() {
#ifdef __riscv
    // Parse and process DTB.
    dtdump(bootp_dtb_req.response->dtb_ptr);
    dtparse(bootp_dtb_req.response->dtb_ptr);
#elif defined(__x86_64__)
    // Initialize ACPI.
    time_init_before_acpi();
    uacpi_status st = uacpi_initialize(0);
    assert_always(st == UACPI_STATUS_OK);
    st = uacpi_namespace_load();
    assert_always(st == UACPI_STATUS_OK);
    // st = uacpi_namespace_initialize();
    // assert_always(st == UACPI_STATUS_OK);
#endif

    // Enumerate PCIe devices.
    dev_filter_t filter = {
        .match_class = true,
        .class       = DEV_CLASS_PCICTL,
    };
    set_t set = device_get_filtered(&filter);
    set_foreach(device_pcictl_t, device, &set) {
        device_pcictl_enumerate(device);
        device_pop_ref(&device->base);
    }
    set_clear(&set);
}

// Reclaim bootloader memory.
void bootp_reclaim_mem() {
    typedef struct {
        size_t base, length;
    } reclaimable_t;
    size_t         reclaimable_len = 0;
    size_t         reclaimable_cap = 0;
    reclaimable_t *reclaimable     = NULL;

    // Collect all reclaimable memory in an array.
    size_t base = 0;
    size_t len  = 0;
    for (uint64_t i = 0; i < bootp_mm_req.response->entry_count; i++) {
        struct limine_memmap_entry *entry = bootp_mm_req.response->entries[i];
        if (entry->type != LIMINE_MEMMAP_BOOTLOADER_RECLAIMABLE) {
            // TODO: LIMINE_MEMMAP_ACPI_RECLAIMABLE is currently not reclaimed.
            continue;
        }
        if (entry->base != base + len) {
            if (len > 16lu * CONFIG_PAGE_SIZE) {
                // logkf_from_isr(LOG_DEBUG, "Adding memory at 0x%{size;x}-0x%{size;x}", base, base + len - 1);
                reclaimable_t ent = {base, len};
                array_lencap_insert(
                    &reclaimable,
                    sizeof(reclaimable_t),
                    &reclaimable_len,
                    &reclaimable_cap,
                    &ent,
                    reclaimable_len
                );
            }
            base = entry->base;
            len  = entry->length;
        } else {
            len += entry->length;
        }
    }
    if (len >= 16lu * CONFIG_PAGE_SIZE) {
        // logkf_from_isr(LOG_DEBUG, "Adding memory at 0x%{size;x}-0x%{size;x}", base, base + len - 1);
        reclaimable_t ent = {base, len};
        array_lencap_insert(
            &reclaimable,
            sizeof(reclaimable_t),
            &reclaimable_len,
            &reclaimable_cap,
            &ent,
            reclaimable_len
        );
    }

    // Reclaim all reclaimable memory.
    for (size_t i = 0; i < reclaimable_len; i++) {
        pmm_mark_free(
            (reclaimable[i].base + CONFIG_PAGE_SIZE - 1) / CONFIG_PAGE_SIZE,
            (reclaimable[i].base + reclaimable[i].length) / CONFIG_PAGE_SIZE
        );
    }
    free(reclaimable);
}

// Send a single character to the log output.
void bootp_early_putc(char msg) {
    // TODO: More proper way to do this.
#ifdef __x86_64__
    outb(0x3f8, msg);
#else
    register char a0 asm("a0") = msg;
    // SBI console putchar.
    asm("li a7, 1; ecall" ::"r"(a0) : "a1", "a7");
#endif
}
