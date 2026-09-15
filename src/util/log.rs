use core::fmt::{Display, Formatter, FormattingOptions, Write};

use crate::kcore::{sync::mutex::RawMutex, timer::time_us};

pub static LOG_MTX: RawMutex = RawMutex::new();

/// Log severity level.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(unused)]
pub enum LogLevel {
    Fatal,
    Error,
    Warning,
    Info,
    Debug,
}

/// Log a formatted message without locking the mutex.
#[macro_export]
macro_rules! logkf_unlocked {
    ($level: expr, $($args:expr),*) => {
        crate::util::log::logkf_unlocked($level, &format_args!($($args),*))
    };
}

/// Log a formatted message.
#[macro_export]
macro_rules! logkf {
    ($level: expr, $($args:expr),*) => {
        crate::util::log::logkf($level, &format_args!($($args),*))
    };
}

/// Print a formatted message.
#[macro_export]
macro_rules! printf {
    ($($args:expr),*) => {
        crate::util::log::printf(&format_args!($($args),*))
    };
}

/// Print a formatted message without locking the mutex.
#[macro_export]
macro_rules! printf_unlocked {
    ($($args:expr),*) => {
        crate::util::log::printf_unlocked(&format_args!($($args),*))
    };
}

/// Log a formatted message without locking the mutex.
pub fn logkf_unlocked(level: LogLevel, thing: &dyn Display) {
    let now = time_us();
    match level {
        LogLevel::Fatal => write_unlocked("\x1b[31m"),
        LogLevel::Error => write_unlocked("\x1b[31m"),
        LogLevel::Warning => write_unlocked("\x1b[33m"),
        LogLevel::Info => write_unlocked("\x1b[32m"),
        LogLevel::Debug => write_unlocked("\x1b[34m"),
    }
    printf_unlocked!("[{:05}.{:03}] ", now / 1000000, now / 1000);
    match level {
        LogLevel::Fatal => write_unlocked("FATAL "),
        LogLevel::Error => write_unlocked("ERROR "),
        LogLevel::Warning => write_unlocked("WARN  "),
        LogLevel::Info => write_unlocked("INFO  "),
        LogLevel::Debug => write_unlocked("DEBUG "),
    }
    printf_unlocked(thing);
    write_unlocked("\x1b[0m\n");
}

/// Log a formatted message.
pub fn logkf(level: LogLevel, thing: &dyn Display) {
    let _guard = LOG_MTX.unintr_timed_lock(10000);
    logkf_unlocked(level, thing);
}

/// Write a string without locking the mutex.
pub fn write_unlocked(thing: &str) {
    // TODO: Replace with proper earlycon.
    let mut prev = 0u8;
    for &c in thing.as_bytes() {
        unsafe {
            if c == b'\n' && prev != b'\r' {
                crate::boot::protocol::bootp_early_putc(b'\r');
            } else if prev == b'\r' && c != b'\n' {
                crate::boot::protocol::bootp_early_putc(b'\n');
            }
            crate::boot::protocol::bootp_early_putc(c);
            prev = c;
        }
    }
}

/// Write a string.
pub fn write(thing: &str) {
    let _guard = LOG_MTX.unintr_timed_lock(10000);
    write_unlocked(thing);
}

/// Write a formatted string without locking the mutex.
pub fn printf_unlocked(thing: &dyn Display) {
    /// Dummy struct used to send format results to the log output.
    struct LogWriter;

    impl Write for LogWriter {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            write_unlocked(s);
            Ok(())
        }
    }

    let _ = thing.fmt(&mut Formatter::new(
        &mut LogWriter {},
        FormattingOptions::default(),
    ));
}

/// Write a formatted string.
pub fn printf(thing: &dyn Display) {
    let _guard = LOG_MTX.unintr_timed_lock(10000);
    printf_unlocked(thing);
}
