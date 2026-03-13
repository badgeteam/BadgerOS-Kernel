
// SPDX-License-Identifier: MIT

#pragma once

#include "attributes.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// PCIe class code.
typedef struct PACKED {
    uint8_t progif;
    uint8_t subclass;
    uint8_t baseclass;
} pci_class_t;



// PCI base class definition.
typedef enum PACKED {
    // PCI base class: Device was built before Class Code definitions were finalized.
    PCI_BCLASS_NULL       = 0x00,
    // PCI base class: Mass storage controller.
    PCI_BCLASS_STORAGE    = 0x01,
    // PCI base class: Network controller.
    PCI_BCLASS_NETIF      = 0x02,
    // PCI base class: Display controller.
    PCI_BCLASS_DISPLAY    = 0x03,
    // PCI base class: Multimedia device.
    PCI_BCLASS_MULTIMEDIA = 0x04,
    // PCI base class: Memory controller.
    PCI_BCLASS_MEMORY     = 0x05,
    // PCI base class: Bridge device.
    PCI_BCLASS_BRIDGE     = 0x06,
    // PCI base class: Simple communication controllers.
    PCI_BCLASS_COMMS      = 0x07,
    // PCI base class: Base system peripherals.
    PCI_BCLASS_BASESYS    = 0x08,
    // PCI base class: Input devices.
    PCI_BCLASS_INPUT      = 0x09,
    // PCI base class: Docking stations.
    PCI_BCLASS_DOCKING    = 0x0A,
    // PCI base class: Processors.
    PCI_BCLASS_PROCESSOR  = 0x0B,
    // PCI base class: Serial bus controllers.
    PCI_BCLASS_SERIAL     = 0x0C,
    // PCI base class: Wireless controller.
    PCI_BCLASS_WIRELESS   = 0x0D,
    // PCI base class: Intelligent I/O controllers.
    PCI_BCLASS_INTIO      = 0x0E,
    // PCI base class: Satellite communication controllers.
    PCI_BCLASS_SATCOMMS   = 0x0F,
    // PCI base class: Encryption/Decryption controllers.
    PCI_BCLASS_CRYPTO     = 0x10,
    // PCI base class: Data acquisition and signal processing controllers.
    PCI_BCLASS_DSP        = 0x11,
    // PCI base class: Processing accelerators.
    PCI_BCLASS_ACCEL      = 0x12,
    // PCI base class: Non-Essential Instrumentation.
    PCI_BCLASS_NEI        = 0x13,
    // PCI base class: Device does not fit in any defined classes.
    PCI_BCLASS_MISC       = 0xFF,
} pci_bclass_t;


#pragma region storage
// PCI subclasses for storage controllers.
typedef enum PACKED {
    // Storage controller subclass: SCSI controllers.
    PCI_SUBCLASS_STORAGE_SCSI     = 0x00,
    // Storage controller subclass: IDE controller.
    PCI_SUBCLASS_STORAGE_IDE      = 0x01,
    // Storage controller subclass: Floppy controller.
    PCI_SUBCLASS_STORAGE_FLOPPY   = 0x02,
    // Storage controller subclass: IPI bus controller.
    PCI_SUBCLASS_STORAGE_IPIBUS   = 0x03,
    // Storage controller subclass: RAID controller.
    PCI_SUBCLASS_STORAGE_RAID     = 0x04,
    // Storage controller subclass: ATA controller with ADMA interface.
    PCI_SUBCLASS_STORAGE_ATA_ADMA = 0x05,
    // Storage controller subclass: Serial ATA controller.
    PCI_SUBCLASS_STORAGE_SATA     = 0x06,
    // Storage controller subclass: Serial-Attached SCSI controller.
    PCI_SUBCLASS_STORAGE_SAS      = 0x07,
    // Storage controller subclass: Non-Volative Memory controller.
    PCI_SUBCLASS_STORAGE_NVM      = 0x08,
    // Storage controller subclass: Universal Flash Storage.
    PCI_SUBCLASS_STORAGE_UFS      = 0x09,
    // Storage controller subclass: Other.
    PCI_SUBCLASS_STORAGE_OTHER    = 0x80,
} pci_subclass_storage_t;

// PCI programming interfaces for SCSI controllers.
typedef enum PACKED {
    // SCSI storage controller interface: Vendor-specific.
    PCI_PROGIF_STORAGE_SCSI_OTHER        = 0x00,
    // SCSI storage controller interface: SCSI storage over PQI.
    PCI_PROGIF_STORAGE_SCSI_PQI_STORAGE  = 0x11,
    // SCSI storage controller interface: SCSI storage and controller over PQI.
    PCI_PROGIF_STORAGE_SCSI_PQI_HYBRID   = 0x12,
    // SCSI storage controller interface: SCSI controller over PQI.
    PCI_PROGIF_STORAGE_SCSI_PQI_CONTROL  = 0x13,
    // SCSI storage controller interface: SCSI over NVMe.
    PCI_PROGIF_STORAGE_SCSI_NVME_STORAGE = 0x21,
} pci_progif_storage_scsi_t;

// PCI programming interfaces for SATA controllers.
typedef enum PACKED {
    // SATA storage controller interface: Vendor-specific.
    PCI_PROGIF_STORAGE_SATA_OTHER = 0x00,
    // SATA storage controller interface: AHCI.
    PCI_PROGIF_STORAGE_SATA_AHCI  = 0x01,
    // SATA storage controller interface: Serial Storage Bus.
    PCI_PROGIF_STORAGE_SATA_SSB   = 0x02,
} pci_progif_storage_sata_t;
#pragma endregion storage
