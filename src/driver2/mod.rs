use crate::dev2::registry::register_driver;

pub mod ns16550;
pub mod pci;
#[cfg(target_arch = "riscv64")]
pub mod riscv_plic;

fn register_drivers() {
    register_driver(&ns16550::Ns16550Driver);
    register_driver(&pci::generic::PciGenericDriver);
    #[cfg(target_arch = "riscv64")]
    register_driver(&riscv_plic::RiscvPlicDriver);
}

register_kmodule! {
    drivers,
    [1, 0, 0],
    register_drivers
}
