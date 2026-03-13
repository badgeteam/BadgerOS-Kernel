
// SPDX-License-Identifier: MIT

#pragma once

#include "cpu/riscv.h"

#include <stdint.h>



// Completed successfully.
#define SBI_SUCCESS               0
// Failed.
#define SBI_ERR_FAILED            -1
// Not supported.
#define SBI_ERR_NOT_SUPPORTED     -2
// Invalid parameter(s).
#define SBI_ERR_INVALID_PARAM     -3
// Denied or not allowed.
#define SBI_ERR_DENIED            -4
// Invalid address(s).
#define SBI_ERR_INVALID_ADDRESS   -5
// Already available.
#define SBI_ERR_ALREADY_AVAILABLE -6
// Already started.
#define SBI_ERR_ALREADY_STARTED   -7
// Already stopped.
#define SBI_ERR_ALREADY_STOPPED   -8
// Shared memory not available.
#define SBI_ERR_NO_SHMEM          -9

// SBI call return value.
typedef struct {
    unsigned long retval;
    unsigned long status;
} sbi_ret_t;

// Define an SBI call function.
#define SBI_CALL(fid, eid, ...)                                                                                        \
    ({                                                                                                                 \
        register unsigned long a0 asm("a0");                                                                           \
        register unsigned long a1 asm("a1");                                                                           \
        register unsigned long a2 asm("a2");                                                                           \
        register unsigned long a3 asm("a3");                                                                           \
        register unsigned long a4 asm("a4");                                                                           \
        register unsigned long a5 asm("a5");                                                                           \
        register unsigned long a6 asm("a6");                                                                           \
        register unsigned long a7 asm("a7");                                                                           \
        a6 = fid;                                                                                                      \
        a7 = eid;                                                                                                      \
        __VA_ARGS__;                                                                                                   \
        asm volatile("ecall" : "+r"(a0), "+r"(a1), "+r"(a2), "+r"(a3), "+r"(a4), "+r"(a5), "+r"(a6), "+r"(a7));        \
        (sbi_ret_t){a0, a1};                                                                                           \
    })



/* ==== SBI BASE EXTENSION ==== */

// Base extension EID.
#define SBI_BASE_EID 0x10

// Get SBI specification version.
static inline sbi_ret_t sbi_get_spec_version() {
    return SBI_CALL(0, SBI_BASE_EID);
}

// Get SBI implementation ID.
static inline sbi_ret_t sbi_get_impl_id() {
    return SBI_CALL(1, SBI_BASE_EID);
}

// Get SBI implementation version.
static inline sbi_ret_t sbi_get_impl_version() {
    return SBI_CALL(2, SBI_BASE_EID);
}

// Probe SBI extension.
static inline sbi_ret_t sbi_probe_extension(long extension_id) {
    return SBI_CALL(3, SBI_BASE_EID, a0 = extension_id);
}

// Get machine vendor ID.
static inline sbi_ret_t sbi_get_mvendorid() {
    return SBI_CALL(4, SBI_BASE_EID);
}

// Get machine architecture ID.
static inline sbi_ret_t sbi_get_marchid() {
    return SBI_CALL(5, SBI_BASE_EID);
}

// Get machine implementation ID.
static inline sbi_ret_t sbi_get_mimpid() {
    return SBI_CALL(6, SBI_BASE_EID);
}



/* ==== SBI legacy extension ==== */

// Set a timer `stime_value` ticks in the future on this HART.
static inline long sbi_legacy_set_timer(uint64_t stime_value) {
#if __riscv_xlen == 32
    register long a0 asm("a0") = (long)stime_value;
    register long a1 asm("a1") = (long)(stime_value >> 32);
    register long a7 asm("a7") = 0x00;
    asm volatile("ecall" : "+r"(a0), "+r"(a1), "+r"(a7));
#else
    register long a0 asm("a0") = stime_value;
    register long a7 asm("a7") = 0x00;
    asm volatile("ecall" : "+r"(a0), "+r"(a7));
#endif
    return a0;
}

// Put a character on the debug console.
static inline long sbi_legacy_console_putchar(int ch) {
    register long a0 asm("a0") = ch;
    register long a7 asm("a7") = 0x01;
    asm volatile("ecall" : "+r"(a0), "+r"(a7));
    return a0;
}

// Read a character from the debug console.
static inline long sbi_legacy_console_getchar() {
    register long a0 asm("a0");
    register long a7 asm("a7") = 0x02;
    asm volatile("ecall" : "=r"(a0), "+r"(a7));
    return a0;
}

// The other legacy SBI functions are not implemented because they are dangerous.
// If the SBI cannot be used to execute remote fence instructions, another mechanism must be used.



/* ==== SBI timer extension ==== */

// Timer extension EID.
#define SBI_TIME_EID 0x54494D45

// Set a timer `stime_value` ticks in the future on this HART.
static inline sbi_ret_t sbi_set_timer(uint64_t stime_value) {
#if __riscv_xlen == 32
    return SBI_CALL(0, SBI_TIME_EID, a0 = (unsigned long)stime_value, a1 = (unsigned long)(stime_value >> 32));
#else
    return SBI_CALL(0, SBI_TIME_EID, a0 = stime_value);
#endif
}



/* ==== SBI inter-processor interrupt extension ==== */

// S-mode IPI extension EID.
#define SBI_IPI_EID 0x735049

// Send an inter-processor interrupt to the specified HARTs.
static inline sbi_ret_t sbi_send_ipi(unsigned long hart_mask, unsigned long hart_base) {
    return SBI_CALL(0, SBI_IPI_EID, a0 = hart_mask, a1 = hart_base);
}



/* ==== SBI remote fence extension ==== */

// Remote fence extension EID.
#define SBI_RFENCE_EID 0x52464E43

// Perform a remote FENCE.I in the specified HARTs.
static inline sbi_ret_t sbi_remote_fence_i(unsigned long hart_mask, unsigned long hart_base) {
    return SBI_CALL(0, SBI_RFENCE_EID, a0 = hart_mask, a1 = hart_base);
}

// Perform a remote SFENCE.VMA for a region of memory in the specified HARTs.
static inline sbi_ret_t sbi_remote_sfence_vma(
    unsigned long hart_mask, unsigned long hart_base, unsigned long start_addr, unsigned long size
) {
    return SBI_CALL(0, SBI_RFENCE_EID, a0 = hart_mask, a1 = hart_base, a2 = start_addr, a3 = size);
}

// Perform a remote SFENCE.VMA for a region of memory with ASID in the specified HARTs.
static inline sbi_ret_t sbi_remote_sfence_vma_asid(
    unsigned long hart_mask, unsigned long hart_base, unsigned long start_addr, unsigned long size, unsigned long asid
) {
    return SBI_CALL(0, SBI_RFENCE_EID, a0 = hart_mask, a1 = hart_base, a2 = start_addr, a3 = size, a4 = asid);
}

// The hypervisor fences are not implemented because BadgerOS does not support virtualization.



/* ==== SBI HART state management extension ==== */

// HART state management extension EID.
#define SBI_HART_MGMT_EID 0x48534D

// HSM extension HART states.
typedef enum {
    // HART is currently running.
    SBI_HART_STARTED,
    // HART is currently stopped.
    SBI_HART_STOPPED,
    // HART is pending startup.
    SBI_HART_STARTING,
    // HART is pending shutdown.
    SBI_HART_STOPPING,
    // HART is suspended.
    SBI_HART_SUSPENDED,
    // HART is pending suspend.
    SBI_HART_SUSPENDING,
    // HART is pending resume.
    SBI_HART_RESUMING,
} sbi_hart_state_t;

// HSM extension suspend types.
typedef enum {
    // Retain CPU state during suspend.
    SBI_SUSPEND_RETAIN,
    // Do not retain CPU state during suspend.
    SBI_SUSPEND_NORETAIN,
} sbi_suspend_t;

// Start a HART at the specified address with a cookie.
// The HART will start in S-mode with VMEM disabled and a0=hartid, a1=cookie.
static inline sbi_ret_t sbi_hart_start(unsigned long hartid, unsigned long entrypoint, unsigned long cookie) {
    return SBI_CALL(0, SBI_HART_MGMT_EID, a0 = hartid, a1 = entrypoint, a2 = cookie);
}

// Stop the execution of the current HART.
static inline sbi_ret_t sbi_hart_stop() {
    return SBI_CALL(1, SBI_HART_MGMT_EID);
}

// Get the current status of a HART.
static inline sbi_ret_t sbi_hart_status(unsigned long hartid) {
    return SBI_CALL(2, SBI_HART_MGMT_EID, a0 = hartid);
}

// Suspend the execution of the current HART.
// If the suspend preserves state, this function returns without error.
// If not, the CPU is restarted much like `sbi_hart_start`.
static inline sbi_ret_t sbi_hart_suspend(uint32_t suspend_type, unsigned long resume_entrypoint, unsigned long cookie) {
    return SBI_CALL(2, SBI_HART_MGMT_EID, a0 = suspend_type, a1 = resume_entrypoint, a2 = cookie);
}
