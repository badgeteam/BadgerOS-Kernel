pub mod ns16550;
pub mod pci;
#[cfg(target_arch = "riscv64")]
pub mod riscv_plic;
pub mod sata;
