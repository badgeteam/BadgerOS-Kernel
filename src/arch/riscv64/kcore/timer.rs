use crate::{
    arch::{kcore::timer::ArchTimer, riscv64::Riscv},
    bindings::log::LogLevel,
};

impl ArchTimer for Riscv {
    fn start_tick_timer() {}

    fn timer_init_dtb(node: &dtb::DtbNode) {
        logkf!(LogLevel::Warning, "TODO: ArchTimer::init_dtb");
    }

    fn time_us() -> u64 {
        0
    }
}
