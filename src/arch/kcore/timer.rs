#[cfg(feature = "dtb")]
use dtb::DtbNode;

// TODO: This entire timer API will be fundamentally re-worked so it is less burden on the arch impl.
pub trait ArchTimer {
    fn start_tick_timer();

    /// Initialize CPU-local timers using DTB information.
    #[cfg(feature = "dtb")]
    fn timer_init_dtb(cpus_node: &DtbNode);

    /// Initialize CPU-local on ACPI systems before tables are loaded.
    #[cfg(feature = "acpi")]
    fn timer_init_pre_acpi();

    /// Initialize CPU-local on ACPI systems after tables are loaded but before AML is loaded.
    #[cfg(feature = "acpi")]
    fn timer_init_acpi();

    /// Get monotonic microsecond timer.
    fn time_us() -> u64;
}
