use tock_registers::{register_bitfields, register_structs, registers::*};

/// Valid FIS types.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
    /// Register FIS - Host to Device
    RegisterH2D = 0x27,
    /// Register FIS - Device to Host
    RegisterD2H = 0x34,
    /// DMA Activate FIS - Device to Host
    DmaActivate = 0x39,
    /// DMA Setup FIS - Bi-directional
    DmaSetup = 0x41,
    /// Data FIS - Bi-directional
    Data = 0x46,
    /// BIST Activate FIS - Bi-directional
    BistActivate = 0x58,
    /// PIO Setup FIS - Device to Host
    PioSetupD2H = 0x5F,
    /// Set Device Bits FIS - Device to Host
    SetDevBitsD2H = 0xA1,
}

impl TryFrom<u8> for Type {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x27 => Ok(Type::RegisterH2D),
            0x34 => Ok(Type::RegisterD2H),
            0x39 => Ok(Type::DmaActivate),
            0x41 => Ok(Type::DmaSetup),
            0x46 => Ok(Type::Data),
            0x58 => Ok(Type::BistActivate),
            0x5F => Ok(Type::PioSetupD2H),
            0xA1 => Ok(Type::SetDevBitsD2H),
            _ => Err(()),
        }
    }
}

// FIS: Register.
register_bitfields! {
    u8,

    /// FIS: Register (host to device): PM port, C.
    pub PMC [
        /// Port multiplier port.
        pm_port   OFFSET(0) NUMBITS(4) [],
        /// Transfer due to update of command register.
        cmdr_xfer OFFSET(7) NUMBITS(1) [],
    ],

    /// FIS: Register (device to host): PM port, I.
    pub PMI [
        /// Port multiplier port.
        pm_port   OFFSET(0) NUMBITS(4) [],
        /// Transfer due to update of command register.
        interrupt OFFSET(6) NUMBITS(1) [],
    ],
}

// FIS: Register.
register_structs! {
    /// FIS: Register (host to device).
    pub RegisterH2D {
        /// FIS type (FisType::RegisterH2D).
        (0x00 => pub fis_type:      ReadWrite<u8>),
        /// PM port, C fields.
        (0x01 => pub pmc:           ReadWrite<u8, PMC::Register>),
        /// Command.
        (0x02 => pub command:       ReadWrite<u8>),
        /// Features.
        (0x03 => pub features:      ReadWrite<u8>),
        /// LBA registers.
        (0x04 => pub lba:           [ReadWrite<u8>; 3]),
        /// Device.
        (0x07 => pub device:        ReadWrite<u8>),
        /// LBA registers expanded.
        (0x08 => pub lba_exp:       [ReadWrite<u8>; 3]),
        /// Features expanded.
        (0x0b => pub features_exp:  ReadWrite<u8>),
        /// Sector count.
        (0x0c => pub sec_count:     ReadWrite<u16>),
        /// Reserved.
        (0x0e => pub _resvd0:       u8),
        /// Control.
        (0x0f => pub control:       ReadWrite<u8>),
        /// Reserved.
        (0x10 => pub _resvd1:       [u8; 4]),
        /// End of structure.
        (0x14 => @END),
    },

    /// FIS: Register (device to host).
    pub RegisterD2H {
        /// FIS type (FisType::RegisterD2H).
        (0x00 => pub fis_type:      ReadWrite<u8>),
        /// PM port, C fields.
        (0x01 => pub pmi:           ReadWrite<u8, PMI::Register>),
        /// Status.
        (0x02 => pub status:        ReadWrite<u8>),
        /// Error.
        (0x03 => pub error:         ReadWrite<u8>),
        /// LBA registers.
        (0x04 => pub lba:           [ReadWrite<u8>; 3]),
        /// Device.
        (0x07 => pub device:        ReadWrite<u8>),
        /// LBA registers expanded.
        (0x08 => pub lba_exp:       [ReadWrite<u8>; 3]),
        /// Reserved.
        (0x0b => pub _resvd0:       u8),
        /// Sector count.
        (0x0c => pub sec_count:     ReadWrite<u16>),
        /// Reserved.
        (0x0e => pub _resvd1:       [u8; 6]),
        /// End of structure.
        (0x14 => @END),
    }
}
