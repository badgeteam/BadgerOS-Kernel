use crate::arch::{kcore::timer::ArchTimer, x86_64::X86_64};

impl ArchTimer for X86_64 {
    fn start_tick_timer() {
        // TODO.
    }

    #[cfg(feature = "dtb")]
    fn timer_init_dtb(_cpus_node: &dtb::DtbNode) {
        todo!()
    }

    fn time_us() -> u64 {
        0
    }
}
