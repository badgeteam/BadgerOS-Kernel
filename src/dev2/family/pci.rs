// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::{bindings::error::EResult, dev2::family::Interface, mem::vmm::kernel_mm};

#[rustfmt::skip]
#[allow(unused_imports)]
pub mod progif {
    use super::baseclass;

    /// Storage programming interfaces (under [`baseclass::STORAGE`]).
    pub mod storage {
        use super::super::subclass;
        
        /// SCSI programming interface (under [`subclass::storage::SCSI`]).
        pub mod scsi {
            /// SCSI storage controller interface: Vendor-specific.
            pub const OTHER: u8        = 0x00;
            /// SCSI storage controller interface: SCSI storage over PQI.
            pub const PQI_STORAGE: u8  = 0x11;
            /// SCSI storage controller interface: SCSI storage and controller over PQI.
            pub const PQI_HYBRID: u8   = 0x12;
            /// SCSI storage controller interface: SCSI controller over PQI.
            pub const PQI_CONTROL: u8  = 0x13;
            /// SCSI storage controller interface: SCSI over NVMe.
            pub const NVME_STORAGE: u8 = 0x21;
        }
        
        /// SATA programming interface (under [`subclass::storage::SATA`]).
        pub mod sata {
            /// SATA storage controller interface: Vendor-specific.
            pub const OTHER: u8 = 0x00;
            /// SATA storage controller interface: AHCI.
            pub const AHCI: u8  = 0x01;
            /// SATA storage controller interface: Serial Storage Bus.
            pub const SSB: u8   = 0x02;
        }
    }
}

#[rustfmt::skip]
#[allow(unused_imports)]
pub mod subclass {
    use super::baseclass;

    /// Storage subclasses (under [`baseclass::STORAGE`]).
    pub mod storage {
        /// Storage controller subclass: SCSI controllers.
        pub const SCSI: u8     = 0x00;
        /// Storage controller subclass: IDE controller.
        pub const IDE: u8      = 0x01;
        /// Storage controller subclass: Floppy controller.
        pub const FLOPPY: u8   = 0x02;
        /// Storage controller subclass: IPI bus controller.
        pub const IPIBUS: u8   = 0x03;
        /// Storage controller subclass: RAID controller.
        pub const RAID: u8     = 0x04;
        /// Storage controller subclass: ATA controller with ADMA interface.
        pub const ATA_ADMA: u8 = 0x05;
        /// Storage controller subclass: Serial ATA controller.
        pub const SATA: u8     = 0x06;
        /// Storage controller subclass: Serial-Attached SCSI controller.
        pub const SAS: u8      = 0x07;
        /// Storage controller subclass: Non-Volative Memory controller.
        pub const NVM: u8      = 0x08;
        /// Storage controller subclass: Universal Flash Storage.
        pub const UFS: u8      = 0x09;
        /// Storage controller subclass: Other.
        pub const OTHER: u8    = 0x80;
    }
}

#[rustfmt::skip]
pub mod baseclass {
    /// PCI base class: Device was built before Class Code definitions were finalized.
    pub const NULL: u8       = 0x00;
    /// PCI base class: Mass storage controller.
    pub const STORAGE: u8    = 0x01;
    /// PCI base class: Network controller.
    pub const NETIF: u8      = 0x02;
    /// PCI base class: Display controller.
    pub const DISPLAY: u8    = 0x03;
    /// PCI base class: Multimedia device.
    pub const MULTIMEDIA: u8 = 0x04;
    /// PCI base class: Memory controller.
    pub const MEMORY: u8     = 0x05;
    /// PCI base class: Bridge device.
    pub const BRIDGE: u8     = 0x06;
    /// PCI base class: Simple communication controllers.
    pub const COMMS: u8      = 0x07;
    /// PCI base class: Base system peripherals.
    pub const BASESYS: u8    = 0x08;
    /// PCI base class: Input devices.
    pub const INPUT: u8      = 0x09;
    /// PCI base class: Docking stations.
    pub const DOCKING: u8    = 0x0A;
    /// PCI base class: Processors.
    pub const PROCESSOR: u8  = 0x0B;
    /// PCI base class: Serial bus controllers.
    pub const SERIAL: u8     = 0x0C;
    /// PCI base class: Wireless controller.
    pub const WIRELESS: u8   = 0x0D;
    /// PCI base class: Intelligent I/O controllers.
    pub const INTIO: u8      = 0x0E;
    /// PCI base class: Satellite communication controllers.
    pub const SATCOMMS: u8   = 0x0F;
    /// PCI base class: Encryption/Decryption controllers.
    pub const CRYPTO: u8     = 0x10;
    /// PCI base class: Data acquisition and signal processing controllers.
    pub const DSP: u8        = 0x11;
    /// PCI base class: Processing accelerators.
    pub const ACCEL: u8      = 0x12;
    /// PCI base class: Non-Essential Instrumentation.
    pub const NEI: u8        = 0x13;
    /// PCI base class: Device does not fit in any defined classes.
    pub const MISC: u8       = 0xFF;
}

/// PCI device class code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClassCode {
    pub progif: u8,
    pub subclass: u8,
    pub baseclass: u8,
}

/// The base PCI device interface.
pub trait PciDevice: Interface {
    /// Get this device's class code.
    fn class_code(&self) -> ClassCode;
    /// Map one of this device's BARs.
    fn map_bar(&self, index: usize) -> EResult<BarMapping>;
}

/// Represents a mapped range of BAR I/O space.
pub struct BarMapping {
    vaddr: usize,
    size: usize,
}
