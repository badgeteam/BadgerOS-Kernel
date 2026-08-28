use tock_registers::{register_bitfields, register_structs, registers::*};

// Bitfields for command header.
register_bitfields! {
    u32,

    /// Command header: DWORD 0; description information.
    pub CmdHdrDesc [
        /// Physical Region Descriptor Table Length (PRDTL): Length of the scatter/gather descriptor
        /// table in entries, called the Physical Region Descriptor Table. Each entry is 4 DW. A '0'
        /// represents 0 entries, FFFFh represents 65,535 entries. The HBA uses this field to know when to
        /// stop fetching PRDs. If this field is '0', then no data transfer shall occur with the command.
        prdtl OFFSET(16) NUMBITS(16) [],

        /// Port Multiplier Port (PMP): Indicates the port number that should be used when constructing
        /// Data FISes on transmit, and to check against all FISes received for this command. This value
        /// shall be set to 0h by software when it has been determined that it is communicating to a directly
        /// attached device.
        pmp OFFSET(12) NUMBITS(4) [],

        /// Clear Busy upon R_OK (CLR_BUSY): When set, the HBA shall clear PxTFD.STS.BSY and
        /// PxCI.CI(pIssueSlot) after transmitting this FIS and receiving R_OK. When cleared, the HBA
        /// shall not clear PxTFD.STS.BSY nor PxCI.CI(pIssueSlot) after transmitting this FIS and receiving
        /// R_OK.
        clr_busy OFFSET(10) NUMBITS(1) [],

        /// BIST (BIST): When '1', indicates that the command that software built is for sending a BIST FIS.
        /// The HBA shall send the FIS and enter a test mode. The tests that can be run in this mode are
        /// outside the scope of this specification.
        bist OFFSET(9) NUMBITS(1) [],

        /// Reset (RESET): When '1', indicates that the command that software built is for a part of a software
        /// reset sequence that manipulates the SRST bit in the Device Control register. The HBA must
        /// perform a SYNC escape (if necessary) to get the device into an idle state before sending the
        /// command. See section 10.4 for details on reset.
        reset OFFSET(8) NUMBITS(1) [],

        /// Prefetchable (PREFETCHABLE): This bit is only valid if the PRDTL field is non-zero or the ATAPI 'A' bit is set
        /// in the command header. When set and PRDTL is non-zero, the HBA may prefetch PRDs in
        /// anticipation of performing a data transfer. When set and the ATAPI 'A' bit is set in the command
        /// header, the HBA may prefetch the ATAPI command. System software shall not set this bit when
        /// using native command queuing commands or when using FIS-based switching with a Port
        /// Multiplier.
        /// Note: The HBA may prefetch the ATAPI command, PRD entries, and data regardless of the
        /// state of this bit. However, it is recommended that the HBA use this information from software to
        /// avoid prefetching needlessly.
        prefetchable OFFSET(7) NUMBITS(1) [],

        /// Write (WRITE): When set, indicates that the direction is a device write (data from system memory to
        /// device). When cleared, indicates that the direction is a device read (data from device to system
        /// memory). If this bit is set and the P bit is set, the HBA may prefetch data in anticipation of
        /// receiving a DMA Setup FIS, a DMA Activate FIS, or PIO Setup FIS, in addition to prefetching
        /// PRDs.
        write OFFSET(6) NUMBITS(1) [],

        /// ATAPI (ATAPI): When '1', indicates that a PIO setup FIS shall be sent by the device indicating a
        /// transfer for the ATAPI command. The HBA may prefetch data from CTBAz[ACMD] in
        /// anticipation of receiving the PIO Setup FIS.
        atapi OFFSET(5) NUMBITS(1) [],

        /// Command FIS Length (CFL): Length of the Command FIS. A '0' represents 0 DW, '4'
        /// represents 4 DW. A length of '0' or '1' is illegal. The maximum value allowed is 10h, or 16 DW.
        /// The HBA uses this field to know the length of the FIS it shall send to the device.
        fis_len OFFSET(0) NUMBITS(5) [],
    ]
}

// Command header layout.
register_structs! {
    /// SATA AHCI command header.
    pub CmdHdr {
        /// "Descriptive information" about the command.
        (0x00 => pub desc:          ReadWrite<u32, CmdHdrDesc::Register>),
        /// PRD byte count.
        (0x04 => pub prd_len:       ReadWrite<u32>),
        /// Command table base address.
        (0x08 => pub cmd_addr_lo:   ReadWrite<u32>),
        /// Command table base address.
        (0x0c => pub cmd_addr_hi:   ReadWrite<u32>),
        /// Reserved.
        (0x10 => _resvd0:           [u32; 4]),
        /// End of structure.
        (0x20 => @END),
    }
}

// Bitfields for physical region descriptor.
register_bitfields! {
    u32,

    pub DBC [
        /// Data byte count.
        count  OFFSET( 0) NUMBITS(22) [],
        /// Interrupt on completion.
        irq_en OFFSET(31) NUMBITS(1) [],
    ]
}

register_structs! {
    /// Physical region descriptor.
    pub PRDT {
        /// Data base address.
        (0x00 => pub paddr: ReadWrite<u64>),
        /// PRD byte count.
        (0x08 => _resvd0:   u32),
        /// Data byte count minus 1.
        (0x0c => pub dbc:   ReadWrite<u32, DBC::Register>),
        /// End of structure.
        (0x10 => @END),
    }
}
