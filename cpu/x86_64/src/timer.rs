// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

// TODO: HPET support

use crate::{bindings::log::LogLevel, config, cpu::ioport};
use core::arch::asm;

use super::cpuid;

/// TSC ticks per second.
static mut TICKS_PER_SEC: u64 = 0;
/// Microseconds per tick in 32.32 fixed-point format.
static mut MICROS_PER_TICK: u64 = 0;
/// Nanoseconds per ticks in 32.32 fixed-point format.
static mut NANOS_PER_TICK: u64 = 0;
/// Offset subtracted from the raw timer value to get time since boot.
static mut BASE_TICK: u64 = 0;
/// Tick interval between the scheduler tick interrupt.
static mut TICK_INTERVAL: u64 = 0;

/// Get the current time in ticks.
#[inline(always)]
fn time_ticks() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!("rdtsc", out("eax")lo, out("edx")hi, options(nomem, nostack, preserves_flags));
    }
    lo as u64 + ((hi as u64) << 32)
}

/// Get the current time in microseconds.
#[unsafe(no_mangle)]
pub extern "C" fn time_us() -> u64 {
    let tick = time_ticks();
    let ratio = unsafe { MICROS_PER_TICK };
    let tmp = tick.widening_mul(ratio);
    (tmp.0 >> 32) | (tmp.1 << 32)
}

/// Get the current time in nanoseconds.
#[unsafe(no_mangle)]
pub extern "C" fn time_ns() -> u64 {
    let tick = time_ticks();
    let ratio = unsafe { NANOS_PER_TICK };
    let tmp = tick.widening_mul(ratio);
    (tmp.0 >> 32) | (tmp.1 << 32)
}

fn calc_tsc_from_pit() -> u64 {
    unsafe {
        ioport::outb(0x43, 0x32);
        ioport::outb(0x40, 0xff);
        ioport::outb(0x40, 0xff);
    }
    let before_tick = time_ticks();
    let mut after_tick = before_tick;

    let mut pit_tick = 0u64;
    while pit_tick < 10_000 {
        after_tick = time_ticks();
        let mut tmp: u16;

        unsafe {
            ioport::outb(0x43, 0);
            tmp = ioport::inb(0x40) as u16;
            tmp |= (ioport::inb(0x40) as u16) << 8;
            tmp = !tmp;
        }

        if tmp < pit_tick as u16 {
            pit_tick += 0x10000;
        }
        pit_tick = (pit_tick & !0xffff) | tmp as u64;
    }
    // The PIT runs at 1.193182 MHz, or one tick every 838.095110385507 nanoseconds.
    let elapsed_nanos = pit_tick * 1_000_000_000 / 1_193_182;
    // let elapsed_nanos = pit_tick * 838_095_110_385 / 1_000_000_000;
    let elapsed_ticks = after_tick - before_tick;

    (elapsed_ticks as u128 * 1_000_000_000 / elapsed_nanos as u128) as u64
}

fn calc_tsc_from_cpuid() -> Option<u64> {
    let tsc_ratio = cpuid(0x15)?;
    if tsc_ratio.eax == 0 || tsc_ratio.ebx == 0 || tsc_ratio.ecx == 0 {
        return None;
    }

    logkf!(LogLevel::Debug, "{:#?}", tsc_ratio);

    Some(tsc_ratio.ecx as u64 * tsc_ratio.ebx as u64 / tsc_ratio.eax as u64)
}

fn init_acpi() {
    // PIT-calibrated TSC frequency.
    let tsc_pit_freq = calc_tsc_from_pit();

    if let Some(tsc_cpuid_freq) = calc_tsc_from_cpuid() {
        let err_percent = tsc_cpuid_freq * 100 / tsc_pit_freq;
        if err_percent > 5 {
            logkf!(
                LogLevel::Warning,
                "TSC frequency according to CPUID differs from PIT-calibrated number by {}%",
                err_percent
            );
            logkf!(
                LogLevel::Warning,
                "Using PIT-calibrated TSC frequency despite CPUID information"
            );
            unsafe { TICKS_PER_SEC = tsc_pit_freq };
        } else {
            logkf!(LogLevel::Info, "Using CPUID-specified TSC frequency");
            unsafe { TICKS_PER_SEC = tsc_cpuid_freq };
        }
    } else {
        logkf!(LogLevel::Info, "Using PIT-calibrated TSC frequency");
        unsafe { TICKS_PER_SEC = tsc_pit_freq };
    }

    logkf!(LogLevel::Info, "TSC frequency: {} Hz", unsafe {
        TICKS_PER_SEC
    });
}

fn init_common() {
    let base_tick = time_ticks();

    // SAFETY: These values are only written to during initialization.
    unsafe {
        BASE_TICK = base_tick;
        TICK_INTERVAL = TICKS_PER_SEC / config::TICKS_PER_SEC as u64;
        MICROS_PER_TICK = (1_000_000 << 32) / TICKS_PER_SEC as u64;
        NANOS_PER_TICK = (1_000_000_000 << 32) / TICKS_PER_SEC as u64;
    }
}

/// Start the timer for the next tick interval.
pub fn start_tick_timer() {
    // SAFETY: These values are only written to during initialization.
    let interval = unsafe { TICK_INTERVAL } as u64;
    if interval == 0 {
        // Don't know how fast the timer is yet so won't start the interrupt.
        return;
    }

    let now = time_ticks();
    let next_tick = now + interval - now % interval;

    // TODO.
}

mod c_api {
    #[unsafe(no_mangle)]
    pub extern "C" fn time_init_before_acpi() {
        super::init_acpi();
    }
}
