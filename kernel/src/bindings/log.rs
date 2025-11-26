use core::{
    ffi::{c_char, c_void},
    fmt::{Display, Formatter, Write},
};

use super::raw;

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

/// Log a hexdump without locking the mutex.
pub fn logk_hexdump_unlocked(level: LogLevel, msg: &str, addr: Option<usize>, data: &[u8]) {
    unsafe {
        raw::logk_len_hexdump_vaddr_from_isr(
            level as u32,
            msg.as_ptr() as *const c_char,
            msg.len(),
            data.as_ptr() as *const c_void,
            data.len(),
            addr.unwrap_or(data.as_ptr() as usize),
        );
    }
}

/// Log a hexdump.
pub fn logk_hexdump(level: LogLevel, msg: &str, addr: Option<usize>, data: &[u8]) {
    unsafe {
        raw::logk_len_hexdump_vaddr(
            level as u32,
            msg.as_ptr() as *const c_char,
            msg.len(),
            data.as_ptr() as *const c_void,
            data.len(),
            addr.unwrap_or(data.as_ptr() as usize),
        );
    }
}

/// Log an unformatted message without locking the mutex.
pub fn logk_unlocked(level: LogLevel, msg: &str) {
    unsafe {
        raw::logk_len_from_isr(level as u32, msg.as_ptr() as *const c_char, msg.len());
    }
}

/// Log an unformatted message.
pub fn logk(level: LogLevel, msg: &str) {
    unsafe {
        raw::logk_len(level as u32, msg.as_ptr() as *const c_char, msg.len());
    }
}

/// Dummy struct used to send format results to the log output.
struct LogWriter {}

impl Write for LogWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        unsafe { raw::rawprint_substr(bytes.as_ptr() as *const c_char, bytes.len()) };
        Ok(())
    }
}

/// Log a formatted message without locking the mutex.
pub fn logkf_unlocked(level: LogLevel, thing: &dyn Display) {
    unsafe {
        raw::logk_prefix(level as u32);
    };
    let mut writer = LogWriter {};
    let mut fmt = Formatter::new(&mut writer, Default::default());
    let _ = thing.fmt(&mut fmt);
    unsafe {
        raw::rawprint(c"\x1b[0m\n".as_ptr());
    }
}

/// Log a formatted message.
pub fn logkf(level: LogLevel, thing: &dyn Display) {
    let acq = unsafe { raw::mutex_acquire(&raw mut raw::log_mtx, raw::LOG_MUTEX_TIMEOUT.into()) };
    logkf_unlocked(level, thing);
    if acq {
        unsafe { raw::mutex_release(&raw mut raw::log_mtx) };
    }
}

/// Write a  string without locking the mutex.
pub fn write_unlocked(thing: &str) {
    unsafe { raw::rawprint_substr(thing.as_ptr() as *const c_char, thing.len()) }
}

/// Write a  string.
pub fn write(thing: &str) {
    let acq = unsafe { raw::mutex_acquire(&raw mut raw::log_mtx, raw::LOG_MUTEX_TIMEOUT.into()) };
    write_unlocked(thing);
    if acq {
        unsafe { raw::mutex_release(&raw mut raw::log_mtx) };
    }
}

/// Write a formatted string without locking the mutex.
pub fn printf_unlocked(thing: &dyn Display) {
    let mut writer = LogWriter {};
    let mut fmt = Formatter::new(&mut writer, Default::default());
    let _ = thing.fmt(&mut fmt);
}

/// Write a formatted string.
pub fn printf(thing: &dyn Display) {
    let acq = unsafe { raw::mutex_acquire(&raw mut raw::log_mtx, raw::LOG_MUTEX_TIMEOUT.into()) };
    printf_unlocked(thing);
    if acq {
        unsafe { raw::mutex_release(&raw mut raw::log_mtx) };
    }
}

/// Log a formatted message without locking the mutex.
#[macro_export]
macro_rules! logkf_unlocked {
    ($level: expr, $($args:expr),*) => {
        crate::bindings::log::logkf_unlocked($level, &format_args!($($args),*))
    };
}

/// Log a formatted message.
#[macro_export]
macro_rules! logkf {
    ($level: expr, $($args:expr),*) => {
        crate::bindings::log::logkf($level, &format_args!($($args),*))
    };
}

/// Write an unformatted message.
pub fn print(msg: &str) {
    unsafe {
        raw::rawprint_substr(msg.as_ptr() as *const c_char, msg.len());
    }
}

/// Print a formatted message.
#[macro_export]
macro_rules! printf {
    ($($args:expr),*) => {
        crate::bindings::log::printf(&format_args!($($args),*))
    };
}
