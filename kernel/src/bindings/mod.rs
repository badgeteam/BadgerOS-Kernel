use bytemuck::{Pod, Zeroable};
use raw::{sigaction, stat_t, timestamp_us_t};

#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(unused)]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(non_upper_case_globals)]
pub mod raw;

pub mod device;

pub mod dlist;
#[macro_use]
pub mod kmodule;
#[macro_use]
pub mod log;
#[macro_use]
pub mod error;
pub mod isr_ctx;
pub mod mutex;
pub mod process;
pub mod semaphore;
pub mod spinlock;
pub mod thread;

pub fn time_us() -> timestamp_us_t {
    unsafe { raw::time_us() }
}

unsafe impl Zeroable for stat_t {}
unsafe impl Pod for stat_t {}
unsafe impl Zeroable for sigaction {}
unsafe impl Pod for sigaction {}
