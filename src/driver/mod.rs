pub mod ata;
#[cfg(feature = "dtb")]
pub mod ns16550;
pub mod pci;
#[cfg(target_arch = "riscv64")]
#[cfg(feature = "dtb")] // TODO: Dependent on probing logic for ACPI RISC-V system.
pub mod riscv_plic;
pub mod sata;
