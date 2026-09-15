use crate::arch::{Arch, kcore::timer::ArchTimer};

/// Get monotonic microsecond time.
#[inline(always)]
pub extern "C" fn time_us() -> u64 {
    Arch::time_us()
}
