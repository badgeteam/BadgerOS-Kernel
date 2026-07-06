// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

//! PCI class codes structured hierarchically.

#![allow(non_upper_case_globals)]

use core::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassCode {
    pub baseclass: u8,
    pub subclass: u8,
    pub progif: u8,
}

impl ClassCode {
    pub const fn new(baseclass: u8, subclass: u8, progif: u8) -> Self {
        Self {
            baseclass,
            subclass,
            progif,
        }
    }
}

impl Display for ClassCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "{:02x}:{:02x}:{:02x}",
            self.baseclass, self.subclass, self.progif
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartialClass {
    pub baseclass: u8,
    pub subclass: u8,
}

impl PartialEq<PartialClass> for ClassCode {
    fn eq(&self, other: &PartialClass) -> bool {
        self.baseclass == other.baseclass && self.subclass == other.subclass
    }
}

impl PartialEq<ClassCode> for PartialClass {
    fn eq(&self, other: &ClassCode) -> bool {
        self.baseclass == other.baseclass && self.subclass == other.subclass
    }
}

macro_rules! class_code_defs {
    ($(
        $(#[$baseattr:meta])*
        $baseclass:ident = $baseval:literal {$(
            $(#[$subattr:meta])*
            $subclass:ident = $subval:literal {$(
                $(#[$progattr:meta])*
                $progif:ident = $progval:literal ;
            )*}
        )*}
    )*) => {$(
        $(#[$baseattr])*
        pub const $baseclass: u8 = $baseval ;

        $(#[$baseattr])*
        pub mod $baseclass {$(
            $(#[$subattr])*
            pub const $subclass: super::PartialClass
                = super::PartialClass { baseclass: $baseval, subclass: $subval };

            $(#[$subattr])*
            pub mod $subclass {$(
                $(#[$progattr])*
                pub const $progif: super::super::ClassCode
                    = super::super::ClassCode { baseclass: $baseval, subclass: $subval, progif: $progval };
            )*}
        )*}
    )*};
}

class_code_defs! {
    /// Device was built before Class Code definitions were finalized.
    null        = 0x00 {}
    /// Mass storage controller.
    storage     = 0x01 {
        /// SCSI controllers.
        scsi        = 0x00 {
            /// Vendor-specific.
            other           = 0x00;
            /// SCSI storage over PQI.
            pqi_storage     = 0x11;
            /// SCSI storage and controller over PQI.
            pqi_hybrid      = 0x12;
            /// SCSI controller over PQI.
            pqi_control     = 0x13;
            /// SCSI over NVMe.
            nvme_storage    = 0x21;
        }
        /// IDE controller.
        ide         = 0x01 {}
        /// Floppy controller.
        floppy      = 0x02 {}
        /// IPI bus controller.
        ipibus      = 0x03 {}
        /// RAID controller.
        raid        = 0x04 {}
        /// ATA controller with ADMA interface.
        ata_adma    = 0x05 {}
        /// Serial ATA controller.
        sata        = 0x06 {
            /// Vendor-specific.
            other   = 0x00;
            /// SATA over AHCI.
            ahci    = 0x01;
            /// Serial Storage Bus.
            ssb     = 0x02;
        }
        /// Serial-Attached SCSI controller.
        sas         = 0x07 {}
        /// Non-Volative Memory controller.
        nvm         = 0x08 {}
        /// Universal Flash Storage.
        ufs         = 0x09 {}
    }
    /// Network controller.
    netif       = 0x02 {}
    /// Display controller.
    display     = 0x03 {}
    /// Multimedia device.
    multimedia  = 0x04 {}
    /// Memory controller.
    memory      = 0x05 {}
    /// Bridge device.
    bridge      = 0x06 {}
    /// Simple communication controllers.
    comms       = 0x07 {}
    /// Base system peripherals.
    basesys     = 0x08 {}
    /// Input devices.
    input       = 0x09 {}
    /// Docking stations.
    docking     = 0x0A {}
    /// Processors.
    processor   = 0x0B {}
    /// Serial bus controllers.
    serial      = 0x0C {
        /// USB controllers.
        usb = 0x03 {
            /// UCHI USB controller.
            uhci = 0x00;
            /// OHCI USB controller.
            ohci = 0x10;
            /// EHCI USB 2 controller
            ehci = 0x20;
            /// xHCI USB 3 controller.
            xhci = 0x30;
            /// USB 4 host bus.
            usb4 = 0x40;
        }
    }
    /// Wireless controller.
    wireless    = 0x0D {}
    /// Intelligent I/O controllers.
    intio       = 0x0E {}
    /// Satellite communication controllers.
    satcomms    = 0x0F {}
    /// Encryption/Decryption controllers.
    crypto      = 0x10 {}
    /// Data acquisition and signal processing controllers.
    dsp         = 0x11 {}
    /// Processing accelerators.
    accel       = 0x12 {}
    /// Non-Essential Instrumentation.
    nei         = 0x13 {}
    /// Device does not fit in any defined classes.
    misc        = 0xFF {}
}
