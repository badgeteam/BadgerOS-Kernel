use ctrl::AHCI_DRIVER;
use port::SATA_DRIVER;

use crate::bindings::device::add_driver;

mod ata;
mod ctrl;
mod fis;
mod hms;
mod port;
mod reg;

/// Add the AHCI drivers.
pub fn add_drivers() {
    let _ = add_driver(&AHCI_DRIVER);
    let _ = add_driver(&SATA_DRIVER.base);
}
