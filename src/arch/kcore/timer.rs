#[cfg(feature = "dtb")]
use dtb::DtbNode;

pub trait ArchTimer {
    fn start_tick_timer();

    /// Initialize CPU-local timers using DTB information.
    #[cfg(feature = "dtb")]
    fn timer_init_dtb(node: &DtbNode);

    /// Get monotonic microsecond timer.
    fn time_us() -> u64;
}
