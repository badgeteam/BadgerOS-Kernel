pub mod sstatus {
    pub const SIE_BIT: u8 = 1;
    pub const SPIE_BIT: u8 = 5;
    pub const UBE_BIT: u8 = 6;
    pub const SPP_BIT: u8 = 8;
    pub const VS_BIT: u8 = 9; // ,10
    pub const FS_BIT: u8 = 13; // ,14
    pub const XS_BIT: u8 = 15; // ,16
    pub const SUM_BIT: u8 = 18;
    pub const MXR_BIT: u8 = 19;

    pub const SIE_MASK: usize = 1 << SIE_BIT;
    pub const SPIE_MASK: usize = 1 << SPIE_BIT;
    pub const UBE_MASK: usize = 1 << UBE_BIT;
    pub const SPP_MASK: usize = 1 << SPP_BIT;
    pub const VS_MASK: usize = 3 << VS_BIT;
    pub const FS_MASK: usize = 3 << FS_BIT;
    pub const XS_MASK: usize = 3 << XS_BIT;
    pub const SUM_MASK: usize = 1 << SUM_BIT;
    pub const MXR_MASK: usize = 1 << MXR_BIT;
}
