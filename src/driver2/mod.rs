pub mod ns16550a;

#[cfg(all(target_arch = "riscv64", feature = "dtb"))]
pub mod riscv_plic;
