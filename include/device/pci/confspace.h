
// SPDX-License-Identifier: MIT

#pragma once

#include "attributes.h"
#include "device/pci/bar.h"
#include "device/pci/class.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// PCIe command register - not meant to be constructed.
typedef union {
    struct {
        // Unused.
        uint16_t          : 2;
        // Bus master / DMA enable.
        uint16_t dma_en   : 1;
        // Does not apply to PCIe.
        uint16_t          : 3;
        // Parity error response.
        uint16_t par_resp : 1;
        // Does not apply to PCIe.
        uint16_t          : 1;
        // SERR# enable.
        uint16_t serr_en  : 1;
        // Does not apply to PCIe.
        uint16_t          : 1;
        // Interrupt disable.
        uint16_t irq_dis  : 1;
    };
    uint16_t val;
} pcie_cmdr_t;

// PCIe status register - not meant to be constructed.
typedef union {
    struct {
        // Unused.
        uint16_t                   : 3;
        // Has extended capabilities list.
        uint16_t extcap            : 1;
        // Does not apply to PCIe.
        uint16_t                   : 2;
        // Master detected parity error.
        uint16_t md_parity_err     : 1;
        // Does not apply to PCIe.
        uint16_t                   : 1;
        // Singalled target abort.
        uint16_t sig_target_abort  : 1;
        // Received target abort.
        uint16_t recv_target_abort : 1;
        // Received master abort.
        uint16_t recv_master_abort : 1;
        // Signalled system error.
        uint16_t sig_sys_err       : 1;
        // Detected parity error.
        uint16_t parity_err        : 1;
    };
    uint16_t val;
} pcie_statr_t;

// PCIe common header layout - not meant to be constructed.
typedef struct {
    // Vendor ID.
    uint16_t              vendor_id;
    // Device ID.
    uint16_t              device_id;
    // Command register.
    VOLATILE pcie_cmdr_t  command;
    // Status register.
    VOLATILE pcie_statr_t status;
    // Revision ID.
    uint8_t               rev;
    // TODO: Figure out what class code means.
    pci_class_t           classcode;
    // Cache line size.
    uint8_t               cache_line_size;
    // Master latency timer (does not apply to PCIe).
    uint8_t               _reserved0;
    // Header type.
    uint8_t               hdr_type;
    // TODO: Figure out what BIST means.
    uint8_t               bist;
} pcie_hdr_com_t;
_Static_assert(sizeof(pcie_hdr_com_t) == 0x10, "Size of `pcie_hdr_com_t` must be 0x10");

// PCIe type 0 (device) header layout - not meant to be constructed.
typedef struct {
    // Header data common to type 0 and type 1.
    pcie_hdr_com_t     common;
    // Base address registers.
    VOLATILE pci_bar_t bar[6];
    // Cardbus CIS pointer.
    uint32_t           cis_ptr;
    // Subsystem vendor ID.
    uint16_t           sub_vendor_id;
    // Subsystem ID.
    uint16_t           sub_id;
    // Expansion ROM base address.
    uint32_t           xrom_base;
    // Capabilities pointer.
    uint8_t            cap_ptr;
    // Reserved.
    uint8_t            _reserved0[7];
    // Interrupt line.
    VOLATILE uint8_t   irq_line;
    // Interrupt pin.
    VOLATILE uint8_t   irq_pin;
    // Does not apply to PCIe.
    uint8_t            _reserved1[2];
} pcie_hdr_dev_t;
_Static_assert(sizeof(pcie_hdr_dev_t) == 0x40, "Size of `pcie_hdr_dev_t` must be 0x40");

// PCIe type 1 (switch) header layout - not meant to be constructed.
typedef struct {
    // Header data common to type 0 and type 1.
    pcie_hdr_com_t    common;
    // Base address registers.
    VOLATILE uint32_t bar[2];
    // Primary bus number (not used by PCIe).
    uint8_t           _reserved0;
    // Secondary bus number.
    uint8_t           secondary_bus;
    // Subordinate bus number.
    uint8_t           subordinate_bus;
    // Secondary latency timer (not used by PCIe).
    uint8_t           _reserved1;
    // I/O base lower 8 bits.
    uint8_t           io_base_lo;
    // I/O limit lower 8 bits.
    uint8_t           io_limit_lo;
    // Secondary status.
    VOLATILE uint16_t secondary_status;
    // Memory base.
    uint16_t          mem_base;
    // Memory limit.
    uint16_t          mem_limit;
    // Prefetchable memory base lower 16 bits.
    uint16_t          pf_base_lo;
    // Prefetchable memory limit lower 16 bits.
    uint16_t          pf_limit_lo;
    // Prefetchable base upper 32 bits.
    uint32_t          pf_base_hi;
    // Prefetchable limit upper 32 bits.
    uint32_t          pf_limit_hi;
    // I/O base upper 16 bits.
    uint16_t          io_base_hi;
    // I/O limit upper 16 bits.
    uint16_t          io_limit_hi;
    // Capabilities pointer.
    uint8_t           cap_ptr;
    // Reserved.
    uint8_t           _reserved2[3];
    // Expansion rom base address.
    uint32_t          xrom_base;
    // Interrupt line.
    VOLATILE uint8_t  irq_line;
    // Interrupt pin.
    VOLATILE uint8_t  irq_pin;
    // Does not apply to PCIe.
    uint8_t           _reserved3[2];
} pcie_hdr_sw_t;
_Static_assert(sizeof(pcie_hdr_sw_t) == 0x40, "Size of `pcie_hdr_sw_t` must be 0x40");

// PCIe capabilities register - not meant to be constructed.
typedef union {
    struct {
        // Capability version, should be 2.
        uint16_t ver     : 3;
        // Device / port type.
        uint16_t type    : 5;
        // Slot implemented.
        uint16_t impl    : 1;
        // Interrupt message number.
        uint16_t irq_msg : 5;
        // Undefined.
        uint16_t         : 1;
        // Reserved.
        uint16_t         : 1;
    };
    uint16_t val;
} pcie_capr_t;

// PCIe device capabilities register - not meant to be constructed.
typedef union {
    struct {
        // Max payload size supported.
        uint32_t max_payload   : 3;
        // Phantom functions supported.
        uint32_t phantom       : 2;
        // Extended tag field supported.
        uint32_t ext_tag_field : 1;
        // Endpoint L0 acceptable latency.
        uint32_t ep0_latency   : 3;
        // Endpoint L1 acceptable latency.
        uint32_t ep1_latency   : 3;
        // Undefined.
        uint32_t               : 3;
        // Role-based error reporting.
        uint32_t role_err      : 1;
        // Reserved.
        uint32_t               : 2;
        // Captured slot power limit value.
        uint32_t power_lim     : 8;
        // Captured slot power limit scale.
        uint32_t power_scale   : 2;
        // Function level reset capability.
        uint32_t func_reset    : 1;
        // Reserved.
        uint32_t               : 3;
    };
    uint32_t val;
} pcie_devcapr_t;

// PCIe device control register - not meant to be constructed.
typedef union {
    struct {
        // Correctable error reporting enable.
        uint16_t corr_err_en      : 1;
        // Non-fatal error reporting enable.
        uint16_t nonfatal_err_en  : 1;
        // Fatal error reporting enable.
        uint16_t fatal_err_en     : 1;
        // Enable relaxed ordering.
        uint16_t relaxed_order_en : 1;
        // Max payload size.
        uint16_t max_payload_size : 3;
        // Extended tag field enable.
        uint16_t ext_tag_field_en : 1;
        // Phantom functions enable.
        uint16_t phantom_en       : 1;
        // Enable no snoop.
        uint16_t nosnoop_en       : 1;
        // Max read request size.
        uint16_t max_read_size    : 3;
        // Bridge configuration retry enable / initiate function level reset.
        uint16_t func_reset_start : 1;
    };
    uint16_t val;
} pcie_devctlr_t;

// PCIe device status register - not meant to be constructed.
typedef union {
    struct {
        // Correctable error detected.
        uint16_t corr_err_det     : 1;
        // Non-fatal error detected.
        uint16_t nonfatal_err_det : 1;
        // Fatal error detected.
        uint16_t fatal_err_det    : 1;
        // Unsupported request detected.
        uint16_t unsupported_req  : 1;
        // AUX power detected.
        uint16_t aux_power_det    : 1;
        // Transactions pending.
        uint16_t trns_pending     : 1;
        // Reserved.
        uint16_t                  : 10;
    };
    uint16_t val;
} pcie_devstatr_t;

// PCIe common capability structure - not meant to be constructed.
typedef struct {
    // Capability ID.
    uint8_t                  cap_id;
    // Next capability pointer.
    uint8_t                  next_cap;
    // PCIe capabilities register.
    pcie_capr_t              capr;
    // PCIe device capabilities register.
    pcie_devcapr_t           devcapr;
    // PCIe device control register.
    VOLATILE pcie_devctlr_t  devctlr;
    // PCIe device status register.
    VOLATILE pcie_devstatr_t devstatr;
} pcie_cap_t;
