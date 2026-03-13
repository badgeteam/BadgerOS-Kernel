use tock_registers::{register_bitfields, register_structs, registers::*};

// Bitfields for generic host control registers.
register_bitfields! {
    u32,

    /// AHCI generic host control: Host Capabilities.
    pub HostCaps [
        /// Number of ports minus one.
        n_ports                OFFSET( 0) NUMBITS(5) [],
        /// Supports external SATA.
        supports_esata         OFFSET( 5) NUMBITS(1) [],
        /// Enclosure management supported.
        supports_enclosure     OFFSET( 6) NUMBITS(1) [],
        /// Command completion coalescing supported.
        supports_cc_coalescing OFFSET( 7) NUMBITS(1) [],
        /// Number of command slots minus one.
        n_cmd_slots            OFFSET( 8) NUMBITS(1) [],
        /// Partial state capable.
        supports_pstate        OFFSET( 9) NUMBITS(1) [],
        /// Slumber state capable.
        supports_slumber       OFFSET(10) NUMBITS(1) [],
        /// Supports multiple DRQ blocks per PIO command.
        pio_multi_drq          OFFSET(11) NUMBITS(1) [],
        /// Supports FIS-based switching.
        supports_fis_switch    OFFSET(12) NUMBITS(1) [],
        /// Supports port multiplier.
        supports_port_mul      OFFSET(13) NUMBITS(1) [],
        /// Supports AHCI only mode.
        supports_ahci_only     OFFSET(14) NUMBITS(1) [],
        /// Reserved
        _resvd0                OFFSET(15) NUMBITS(1) [],
        /// Interface speed support.
        max_if_speed           OFFSET(16) NUMBITS(4) [],
        /// Supports command list override.
        supports_clo           OFFSET(20) NUMBITS(1) [],
        /// Supports activity LED.
        supports_led           OFFSET(21) NUMBITS(1) [],
        /// Supports aggressive link power management.
        supports_alp           OFFSET(22) NUMBITS(1) [],
        /// Supports staggered spin-up.
        supports_ss            OFFSET(23) NUMBITS(1) [],
        /// Supports mechanical presence switch.
        supports_det_sw        OFFSET(24) NUMBITS(1) [],
    ],

    /// AHCI generic host control: Global Host Control.
    pub HostCtrl [
        /// HBA reset.
        hba_reset         OFFSET(0)  NUMBITS(1) [],
        /// Interrupt enable.
        irq_en            OFFSET(1)  NUMBITS(1) [],
        /// HBA reverted to single MSI mode.
        single_msi_mode   OFFSET(2)  NUMBITS(1) [],
        /// Reserved.
        _resvd0           OFFSET(3)  NUMBITS(28) [],
        /// Legacy AHCI enable.
        ahci_en           OFFSET(31) NUMBITS(1) [],
    ],

    /// AHCI generic host control: Version.
    pub HostVer [
        /// Major version.
        major OFFSET(0)  NUMBITS(16) [],
        /// Minor version.
        minor OFFSET(16) NUMBITS(16) [],
    ],

    /// AHCI generic host control: Command Completion Coalescing Control.
    pub HostCoalesc [
        // Enable coalescing of command completion interrupts.
        enable        OFFSET(0) NUMBITS(1) [],
        // Reserved.
        _resvd0       OFFSET(1) NUMBITS(2) [],
        // Interrupt to use for this feature.
        irq           OFFSET(3) NUMBITS(5) [],
        // Number of command completions before an interrupt.
        n_completions OFFSET(8) NUMBITS(1) [],
        // Timeout in 1 millisecond interval.
        timeout_ms    OFFSET(9) NUMBITS(1) [],
    ],

    /// AHCI generic host control: Host Capabilities Extended.
    pub HostCapsExt [
        /// Supports BIOS/OS handoff.
        supports_bios_handoff      OFFSET(0) NUMBITS(1) [],
        /// Supports NVMHCI.
        supports_nvmhci            OFFSET(1) NUMBITS(1) [],
        /// Supports automatic partial to slumber transitions.
        supports_auto_slumber      OFFSET(2) NUMBITS(1) [],
        /// Supports device sleep.
        supports_dev_sleep         OFFSET(3) NUMBITS(1) [],
        /// Supports aggressive device sleep management.
        supports_adm               OFFSET(4) NUMBITS(1) [],
        /// Only allows device sleep from slumber.
        sleep_from_slumber_only    OFFSET(5) NUMBITS(1) [],
    ],

    /// AHCI generic host control: BIOS/OS Handoff Control and Status.
    pub HostBOHC [
        /// BIOS owned.
        bios_owned     OFFSET(0) NUMBITS(1) [],
        /// OS owned.
        os_owned       OFFSET(1) NUMBITS(1) [],
        /// SMI on ownership change.
        ooc_smi        OFFSET(2) NUMBITS(1) [],
        /// OS ownership change.
        ooc            OFFSET(3) NUMBITS(1) [],
        /// BIOS busy.
        bios_busy      OFFSET(4) NUMBITS(1) [],
    ],
}

// Generic host control registers.
register_structs! {
    /// HBA BAR registers: generic host control.
    pub Host {
        /// Host Capabilities.
        (0x00 => pub cap:        ReadOnly <u32, HostCaps::Register>),
        /// Global Host Control.
        (0x04 => pub ghc:        ReadWrite<u32, HostCtrl::Register>),
        /// Interrupt Status.
        (0x08 => pub irq_status: ReadWrite<u32>),
        /// Ports Implemented.
        (0x0c => pub ports_impl: ReadOnly <u32>),
        /// Version.
        (0x10 => pub version:    ReadOnly <u32, HostVer::Register>),
        /// Command Completion Coalescing Control.
        (0x14 => pub ccc_ctl:    ReadWrite<u32, HostCoalesc::Register>),
        /// Command Completion Coalsecing Ports.
        (0x18 => pub ccc_ports:  ReadWrite<u32>),
        /// Enclosure Management Location.
        (0x1c => pub em_loc:     ReadOnly <u32>),
        /// Enclosure Management Control.
        (0x20 => pub em_ctl:     ReadOnly <u32>),
        /// Host Capabilities Extended.
        (0x24 => pub cap2:       ReadOnly <u32, HostCapsExt::Register>),
        /// BIOS/OS Handoff Control and Status.
        (0x28 => pub bohc:       ReadWrite<u32, HostBOHC::Register>),
        /// End of structure.
        (0x2c => @END),
    }
}

// bitfields for port registers.
register_bitfields! {
    u32,

    /// AHCI port interrupt status.
    pub PortIrq [
        /// Device to host register FIS interrupt.
        d2h_reg_fis   OFFSET(0)  NUMBITS(1) [],
        /// PIO setup FIS interrupt.
        pio_setup_fis OFFSET(1)  NUMBITS(1) [],
        /// DMA setup FIS interrupt.
        dma_setup_fis OFFSET(2)  NUMBITS(1) [],
        /// Set device bits interrupt.
        set_dev_bits  OFFSET(3)  NUMBITS(1) [],
        /// Unknown FIS interrupt.
        unknown_fis   OFFSET(4)  NUMBITS(1) [],
        /// Descriptor processed.
        desc_proc     OFFSET(5)  NUMBITS(1) [],
        /// Port connect status change.
        port_status   OFFSET(6)  NUMBITS(1) [],
        /// Device mechanical presence status.
        presence      OFFSET(7)  NUMBITS(1) [],
        /// PhyRdy signal changed.
        phy_rdy       OFFSET(22) NUMBITS(1) [],
        /// Incorrect port multiplier status.
        port_mul_err  OFFSET(23) NUMBITS(1) [],
        /// Overflow status.
        overflow      OFFSET(24) NUMBITS(1) [],
        /// Interface non-fatal error.
        if_nonfatal   OFFSET(26) NUMBITS(1) [],
        /// Fatal error that caused the transfer to abort.
        if_fatal      OFFSET(27) NUMBITS(1) [],
        /// Host bus data error.
        hb_data_err   OFFSET(28) NUMBITS(1) [],
        /// Host bus fatal error.
        hb_fatal_err  OFFSET(29) NUMBITS(1) [],
        /// Task file error status.
        tf_err        OFFSET(30) NUMBITS(1) [],
        /// Connected status changed.
        conn_changed  OFFSET(31) NUMBITS(1) [],
    ],

    /// AHCI port command and status register.
    pub PortCmd [
        /// Start command list.
        cmd_start               OFFSET(0)  NUMBITS(1) [],
        /// Spin up device.
        spinup                  OFFSET(1)  NUMBITS(1) [],
        /// Power on device.
        poweron                 OFFSET(2)  NUMBITS(1) [],
        /// Command list override.
        clo                     OFFSET(3)  NUMBITS(1) [],
        /// FIS receive enable.
        fis_en                  OFFSET(4)  NUMBITS(1) [],
        /// Reserved.
        _resvd0                 OFFSET(5)  NUMBITS(3) [],
        /// Current command slot being issued.
        cur_slot                OFFSET(8)  NUMBITS(5) [],
        /// Presence switch state.
        present_switch          OFFSET(13) NUMBITS(1) [],
        /// FIS receive DMA engine is running.
        fis_running             OFFSET(14) NUMBITS(1) [],
        /// Command list running.
        cmd_running             OFFSET(15) NUMBITS(1) [],
        /// Cold presence state.
        present_cold            OFFSET(16) NUMBITS(1) [],
        /// Port multiplier attached.
        port_mul_det            OFFSET(17) NUMBITS(1) [],
        /// Hot plug capable port.
        supports_hotplug        OFFSET(18) NUMBITS(1) [],
        /// Port has mechanical presence switch.
        supports_present_switch OFFSET(19) NUMBITS(1) [],
        /// Supports cold presence detection.
        supports_present_cold   OFFSET(20) NUMBITS(1) [],
        /// Is an external port.
        is_external             OFFSET(21) NUMBITS(1) [],
        /// FIS-based switching capable.
        supports_fis_switching  OFFSET(22) NUMBITS(1) [],
        /// Enable automatic partial to slumber.
        auto_slumber_en         OFFSET(23) NUMBITS(1) [],
        /// Is an ATAPI device.
        dev_is_atapi            OFFSET(24) NUMBITS(1) [],
        /// Enable drive LED even for ATAPI devices.
        drive_led_atapi         OFFSET(25) NUMBITS(1) [],
        /// Interface communication control.
        if_comm_ctrl            OFFSET(28) NUMBITS(4) [
            /// The HBA is ready to accept a new change.
            NOP       = 0,
            /// Change into the active state.
            ACTIVE    = 1,
            /// Change into the partial state; may be rejected by the device.
            PARTIAL   = 2,
            /// Assert the DEVSLP signal, causing the device to sleep.
            DEV_SLEEP = 8
        ],
    ],

    /// AHCI port task file data.
    pub PortTFD [
        /// Status.
        status OFFSET(0)  NUMBITS(8) [],
        /// Error.
        err    OFFSET(8)  NUMBITS(8) [],
        /// Reserved.
        _resvd0 OFFSET(16) NUMBITS(16) [],
    ],

    /// AHCI port SATA status register.
    pub PortSStatus [
        /// Device detection.
        detect OFFSET(0) NUMBITS(4) [],
        /// Current interface speed.
        speed  OFFSET(4) NUMBITS(4) [],
        /// Interface power management.
        power  OFFSET(8) NUMBITS(4) [],
        /// Reserved.
        _resvd0 OFFSET(12) NUMBITS(20) [],
    ],

    /// AHCI port SATA control register.
    pub PortSCtrl [
        /// Device detection initiation.
        det       OFFSET(0) NUMBITS(4) [],
        /// Maximum speed.
        max_speed OFFSET(4) NUMBITS(4) [],
        /// Allowed power management transitions.
        power     OFFSET(8) NUMBITS(4) [],
        /// Reserved.
        _resvd0   OFFSET(12) NUMBITS(20) [],
    ],

    /// AHCI port SATA error register.
    pub PortSError [
        /// Error code.
        err  OFFSET(0)  NUMBITS(16) [],
        /// Diagnostics code.
        diag OFFSET(16) NUMBITS(16) [],
    ],
}

// HBA BAR registers: AHCI port.
register_structs! {
    /// HBA BAR registers: AHCI port.
    pub Port {
        /// Command list base address (low 32 bits).
        (0x00 => pub cmdlist_addr_lo: ReadWrite<u32>),
        /// Command list base address (high 32 bits).
        (0x04 => pub cmdlist_addr_hi: ReadWrite<u32>),
        /// FIS base address (low 32 bits).
        (0x08 => pub fis_addr_lo:     ReadWrite<u32>),
        /// FIS base address (high 32 bits).
        (0x0c => pub fis_addr_hi:     ReadWrite<u32>),
        /// Interrupt status.
        (0x10 => pub irq_status:      ReadWrite<u32, PortIrq::Register>),
        /// Interrupt enable.
        (0x14 => pub irq_enable:      ReadWrite<u32, PortIrq::Register>),
        /// Command and status register.
        (0x18 => pub cmd:             ReadWrite<u32, PortCmd::Register>),
        /// Reserved.
        (0x1c => pub _resvd0:         ReadOnly<u32>),
        /// Task file data.
        (0x20 => pub tfd:             ReadWrite<u32, PortTFD::Register>),
        /// Port signature.
        (0x24 => pub signature:       ReadOnly<u32>),
        /// SATA status register.
        (0x28 => pub sstatus:         ReadWrite<u32, PortSStatus::Register>),
        /// SATA control register.
        (0x2c => pub sctrl:           ReadWrite<u32, PortSCtrl::Register>),
        /// SATA error register.
        (0x30 => pub err:             ReadWrite<u32, PortSError::Register>),
        /// SATA active mask.
        (0x34 => pub active:          ReadWrite<u32>),
        /// Command issue.
        (0x38 => pub cmd_issue:       ReadWrite<u32>),
        /// Port multiplier notification management.
        (0x3c => pub notif:           ReadWrite<u32>),
        /// Port multiplier switching control.
        (0x40 => pub swctrl:          ReadWrite<u32>),
        /// Device sleep control.
        (0x44 => pub sleep_ctrl:      ReadWrite<u32>),
        /// End of structure.
        (0x48 => @END),
    }
}
