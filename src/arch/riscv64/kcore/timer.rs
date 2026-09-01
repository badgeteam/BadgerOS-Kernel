use core::arch::asm;

use crate::{
    arch::{
        kcore::timer::ArchTimer,
        riscv64::{Riscv, csr, sbi},
    },
    bindings::log::LogLevel,
    config,
};

/// Whether the new SBI timer extension is in use.
static mut SUPPORTS_SBI_TIME: bool = false;
/// Raw timer ticks that happen per second.
static mut TICKS_PER_SEC: u64 = 0;
/// Microseconds per tick in 32.32 fixed-point format.
static mut MICROS_PER_TICK: u64 = 0;
/// Nanoseconds per ticks in 32.32 fixed-point format.
static mut NANOS_PER_TICK: u64 = 0;
/// Offset subtracted from the raw timer value to get time since boot.
static mut BASE_TICK: u64 = 0;
/// Tick interval between the scheduler tick interrupt.
static mut TICK_INTERVAL: u64 = 0;

fn time_ticks() -> u64 {
    let ticks: u64;
    unsafe { asm!("rdtime {}", out(reg) ticks) };
    // SAFETY: BASE_TICK is only written to during initialization.
    ticks - unsafe { BASE_TICK }
}

impl ArchTimer for Riscv {
    fn start_tick_timer() {
        // SAFETY: These values are only written to during initialization.
        let interval = unsafe { TICK_INTERVAL } as u64;
        if interval == 0 {
            // Don't know how fast the timer is yet so won't start the interrupt.
            return;
        }

        let now = time_ticks();
        let next_tick = now + interval - now % interval;

        if unsafe { SUPPORTS_SBI_TIME } {
            sbi::timer::set_timer(next_tick + unsafe { BASE_TICK }).unwrap();
        } else {
            sbi::legacy::set_timer(next_tick - now).unwrap();
        }

        // SAFETY: This enables the timer interrupt, not interrupts globally.
        unsafe { asm!("csrs sie, {}", in(reg)csr::sip_sie::STIP_MASK) };
    }

    #[cfg(feature = "dtb")]
    fn timer_init_dtb(cpus_node: &dtb::DtbNode) {
        let timebase_freq = cpus_node
            .prop("timebase-frequency")
            .expect("Missing DTB prop /cpus/timebase-frequency");
        // SAFETY: This value is only written to during initialization.
        unsafe {
            TICKS_PER_SEC = timebase_freq
                .read_uint()
                .expect("Malformed DTB prop /cpus/timebase-frequency")
                as u64
        };
        init_common();
    }

    fn time_us() -> u64 {
        let tick = time_ticks();
        let ratio = unsafe { MICROS_PER_TICK };
        let tmp = tick.widening_mul(ratio);
        (tmp.0 >> 32) | (tmp.1 << 32)
    }
}

fn init_common() {
    let sbi_time = sbi::timer::probe();
    let base_tick = time_ticks();

    // SAFETY: These values are only written to during initialization.
    unsafe {
        SUPPORTS_SBI_TIME = sbi_time;
        BASE_TICK = base_tick;
        TICK_INTERVAL = TICKS_PER_SEC / config::TICKS_PER_SEC as u64;
        MICROS_PER_TICK = (1_000_000 << 32) / TICKS_PER_SEC as u64;
        NANOS_PER_TICK = (1_000_000_000 << 32) / TICKS_PER_SEC as u64;
    }

    if sbi_time {
        logkf!(LogLevel::Info, "Using new SBI timer");
    } else {
        logkf!(LogLevel::Info, "Using legacy SBI timer");
    }
}
