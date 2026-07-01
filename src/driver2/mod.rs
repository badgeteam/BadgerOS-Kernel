use crate::dev2::registry;

pub mod ns16550a;

#[cfg(all(target_arch = "riscv64", feature = "dtb"))]
pub mod riscv_plic;

fn register_drivers() {
    registry::register_driver(&ns16550a::Ns16550aDriver);

    #[cfg(all(target_arch = "riscv64", feature = "dtb"))]
    registry::register_driver(&riscv_plic::RiscvPlicDriver);
}

register_kmodule! {
    drivers,
    [1, 0, 0],
    register_drivers
}
