
// SPDX-License-Identifier: MIT

#include "time.h"

#include "assertions.h"
#include "cpu/x86_cpuid.h"
#include "cpu/x86_ioport.h"
#include "interrupt.h"
#include "scheduler/isr.h"
#include "time_private.h"



// Ticks per second for the LAPIC.
static uint32_t lapic_ticks_per_sec;
// Ticks per second for the TSC.
static uint32_t tsc_ticks_per_sec;
// Tick offset for the purpose of timekeeping.
static uint64_t base_tick;
// Use HPET (instead of legacy PIT).
static bool     use_hpet;


// Get the current time in ticks.
__attribute__((always_inline)) static inline uint64_t time_ticks() {
    // uint32_t lo;
    // uint32_t hi;
    // asm("rdtsc" : "=a"(lo), "=d"(hi));
    return __builtin_ia32_rdtsc();
}

// Set the timer for a certain timestamp.
static void set_timer_ticks(uint64_t timestamp) {
}

// Set the CPU's timer to a certain timestamp.
void time_set_cpu_timer(timestamp_us_t timestamp) {
    set_timer_ticks(timestamp * lapic_ticks_per_sec / 1000000);
}

// Clear the CPU's timer.
void time_clear_cpu_timer() {
}



// Calibrate TSC.
static void cal_tsc() {
    // Start PIT.
    uint64_t pit_tick = 0;
    outb(0x43, 0x32);
    outb(0x40, 0xff);
    outb(0x40, 0xff);
    base_tick = time_ticks();
    uint64_t after_tick;

    while (pit_tick < 100000) {
        after_tick = time_ticks();
        outb(0x43, 0);
        uint16_t tmp  = 0;
        tmp          |= inb(0x40);
        tmp          |= inb(0x40) << 8;
        tmp           = ~tmp;
        if (tmp < (uint16_t)pit_tick) {
            pit_tick += 0x10000;
        }
        pit_tick = (pit_tick & ~0xffff) | tmp;
    }

    uint64_t elapsed_ns = pit_tick * 838095344 / 2000000000;
    logkf(
        LOG_DEBUG,
        "Elapsed PIT ticks: %{u64;d}, nanos: %{u64;d}, TSC ticks %{u64;d}",
        pit_tick,
        elapsed_ns,
        after_tick - base_tick
    );
    tsc_ticks_per_sec = (after_tick - base_tick) * 1000000000 / elapsed_ns;
    logkf(LOG_DEBUG, "TSC ticks per second: %{u64;d}", tsc_ticks_per_sec);
}

// Early timer init before ACPI (but not DTB).
void time_init_before_acpi() {
    cpuid_t freq = cpuid(0x15);
    if (freq.eax && freq.ebx && freq.ecx) {
        logkf(LOG_DEBUG, "Crystal: %{u32;d}, ratio: %{u32;d}/%{u32;d}", freq.ecx, freq.ebx, freq.eax);
        tsc_ticks_per_sec = (uint64_t)freq.ecx * freq.ebx / freq.eax;
        logkf(LOG_DEBUG, "TSC ticks per second: %{u64;d}", tsc_ticks_per_sec);
    } else {
        cal_tsc();
    }

    // logk(LOG_DEBUG, "waiting for a bit");
    // timestamp_us_t lim = time_us() + 60000000;
    // while (time_us() < lim - 5000000);
    // logk(LOG_DEBUG, "almost there");
    // while (time_us() < lim);
    // logk(LOG_DEBUG, "done");

    // Finally, run generic timer init code.
    time_init_generic();
}

// Initialise timer using ACPI.
void time_init_acpi() {
}

// Get current time in microseconds.
timestamp_us_t time_us() {
    if (!tsc_ticks_per_sec) {
        return 0;
    }
    return (time_ticks() - base_tick) * 1000000 / tsc_ticks_per_sec / 60 * 27;
}
