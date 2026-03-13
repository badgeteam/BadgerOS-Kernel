/// ATA command types.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Command {
    Nop = 0x00,
    DataSetMgmt = 0x06,
    DevReset = 0x08,
    ReqSenseDataExt = 0x0b,
    ReadDma = 0xc8,
    ReadDmaExt = 0x25,
    WriteDma = 0xca,
    WriteDmaExt = 0x35,
    FlushCache = 0xe7,
    FlushCacheExt = 0xea,
    IdentDev = 0xec,
}
