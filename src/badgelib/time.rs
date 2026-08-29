// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{
    badgelib::irq::IrqGuard,
    bindings::{self, spinlock::Spinlock},
    process::uapi::time::timespec,
};

// TODO: Replace this code with the chrono crate where possible.

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Debug)]
/// Posix nanoseconds timestamp.
pub struct Timespec {
    /// Seconds (excluding leap) since 00:00, Jan 1 1970 UTC.
    pub sec: u64,
    /// Nanoseconds after [`Self::sec`].
    pub nsec: u32,
}

impl Timespec {
    pub fn now() -> Self {
        // TODO: Use actual RTC time instead of time since boot with kernel compile time added.
        let micros = unsafe { bindings::raw::time_us() } as u64 + compile_time::unix!() * 1000000;
        Self {
            sec: micros / 1000000,
            nsec: (micros % 1000000) as u32 * 1000,
        }
    }
}

impl Into<timespec> for Timespec {
    fn into(self) -> timespec {
        timespec {
            tv_sec: self.sec as i64,
            tv_nsec: self.nsec as i64,
        }
    }
}

/// Atomic version of [`Timespec`].
pub struct AtomicTimespec(Spinlock<Timespec>);

impl AtomicTimespec {
    pub fn new(time: Timespec) -> Self {
        Self(Spinlock::new(time))
    }

    pub fn load(&self) -> Timespec {
        let _guard = IrqGuard::new();
        let tmp = *self.0.lock_shared();
        tmp
    }

    pub fn store(&self, value: Timespec) {
        let _guard = IrqGuard::new();
        *self.0.lock() = value;
    }
}
