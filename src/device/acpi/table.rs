use core::{marker::PhantomData, mem::offset_of};

use crate::uacpi_sys::*;

#[derive(Clone, Copy)]
pub enum MadtEntry<'a> {
    Lapic(&'a acpi_madt_lapic),
    Ioapic(&'a acpi_madt_ioapic),
    InterruptSourceOverride(&'a acpi_madt_interrupt_source_override),
    NmiSource(&'a acpi_madt_nmi_source),
    LapicNmi(&'a acpi_madt_lapic_nmi),
    LapicAddressOverride(&'a acpi_madt_lapic_address_override),
    Iosapic(&'a acpi_madt_iosapic),
    Lsapic(&'a acpi_madt_lsapic),
    PlatformInterruptSources(&'a acpi_madt_platform_interrupt_source),
    LocalX2apic(&'a acpi_madt_x2apic),
    LocalX2apicNmi(&'a acpi_madt_x2apic_nmi),
    Gicc(&'a acpi_madt_gicc),
    Gicd(&'a acpi_madt_gicd),
    GicMsiFrame(&'a acpi_madt_gic_msi_frame),
    Gicr(&'a acpi_madt_gicr),
    GicIts(&'a acpi_madt_gic_its),
    MultiprocessorWakeup(&'a acpi_madt_multiprocessor_wakeup),
    CorePic(&'a acpi_madt_core_pic),
    LioPic(&'a acpi_madt_lio_pic),
    HtPic(&'a acpi_madt_ht_pic),
    EioPic(&'a acpi_madt_eio_pic),
    MsiPic(&'a acpi_madt_msi_pic),
    BioPic(&'a acpi_madt_bio_pic),
    LpcPic(&'a acpi_madt_lpc_pic),
    Rintc(&'a acpi_madt_rintc),
    Imsic(&'a acpi_madt_imsic),
    Aplic(&'a acpi_madt_aplic),
    Plic(&'a acpi_madt_plic),
    Unknown(&'a [u8]),
}

#[derive(Clone, Copy)]
pub struct MadtEntries<'a> {
    ptr: *const acpi_entry_hdr,
    remaining: usize,
    marker: PhantomData<&'a acpi_madt>,
}

impl<'a> MadtEntries<'a> {
    /// # Safety
    /// The caller promises that the MADT is well-formed.
    pub const unsafe fn new(madt: &'a acpi_madt) -> Self {
        Self {
            ptr: &raw const madt.entries as _,
            remaining: madt.hdr.length as usize - offset_of!(acpi_madt, entries),
            marker: PhantomData,
        }
    }
}

impl<'a> Iterator for MadtEntries<'a> {
    type Item = MadtEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        use MadtEntry::*;
        if self.remaining == 0 {
            return None;
        }

        unsafe {
            let hdr = *self.ptr;

            let res = match hdr.type_ as acpi_madt_entry_type {
                ACPI_MADT_ENTRY_TYPE_LAPIC => Lapic(&*(self.ptr as *const acpi_madt_lapic)),
                ACPI_MADT_ENTRY_TYPE_IOAPIC => Ioapic(&*(self.ptr as *const acpi_madt_ioapic)),
                ACPI_MADT_ENTRY_TYPE_INTERRUPT_SOURCE_OVERRIDE => InterruptSourceOverride(
                    &*(self.ptr as *const acpi_madt_interrupt_source_override),
                ),
                ACPI_MADT_ENTRY_TYPE_NMI_SOURCE => {
                    NmiSource(&*(self.ptr as *const acpi_madt_nmi_source))
                }
                ACPI_MADT_ENTRY_TYPE_LAPIC_NMI => {
                    LapicNmi(&*(self.ptr as *const acpi_madt_lapic_nmi))
                }
                ACPI_MADT_ENTRY_TYPE_LAPIC_ADDRESS_OVERRIDE => {
                    LapicAddressOverride(&*(self.ptr as *const acpi_madt_lapic_address_override))
                }
                ACPI_MADT_ENTRY_TYPE_IOSAPIC => Iosapic(&*(self.ptr as *const acpi_madt_iosapic)),
                ACPI_MADT_ENTRY_TYPE_LSAPIC => Lsapic(&*(self.ptr as *const acpi_madt_lsapic)),
                ACPI_MADT_ENTRY_TYPE_PLATFORM_INTERRUPT_SOURCES => PlatformInterruptSources(
                    &*(self.ptr as *const acpi_madt_platform_interrupt_source),
                ),
                ACPI_MADT_ENTRY_TYPE_LOCAL_X2APIC => {
                    LocalX2apic(&*(self.ptr as *const acpi_madt_x2apic))
                }
                ACPI_MADT_ENTRY_TYPE_LOCAL_X2APIC_NMI => {
                    LocalX2apicNmi(&*(self.ptr as *const acpi_madt_x2apic_nmi))
                }
                ACPI_MADT_ENTRY_TYPE_GICC => Gicc(&*(self.ptr as *const acpi_madt_gicc)),
                ACPI_MADT_ENTRY_TYPE_GICD => Gicd(&*(self.ptr as *const acpi_madt_gicd)),
                ACPI_MADT_ENTRY_TYPE_GIC_MSI_FRAME => {
                    GicMsiFrame(&*(self.ptr as *const acpi_madt_gic_msi_frame))
                }
                ACPI_MADT_ENTRY_TYPE_GICR => Gicr(&*(self.ptr as *const acpi_madt_gicr)),
                ACPI_MADT_ENTRY_TYPE_GIC_ITS => GicIts(&*(self.ptr as *const acpi_madt_gic_its)),
                ACPI_MADT_ENTRY_TYPE_MULTIPROCESSOR_WAKEUP => {
                    MultiprocessorWakeup(&*(self.ptr as *const acpi_madt_multiprocessor_wakeup))
                }
                ACPI_MADT_ENTRY_TYPE_CORE_PIC => CorePic(&*(self.ptr as *const acpi_madt_core_pic)),
                ACPI_MADT_ENTRY_TYPE_LIO_PIC => LioPic(&*(self.ptr as *const acpi_madt_lio_pic)),
                ACPI_MADT_ENTRY_TYPE_HT_PIC => HtPic(&*(self.ptr as *const acpi_madt_ht_pic)),
                ACPI_MADT_ENTRY_TYPE_EIO_PIC => EioPic(&*(self.ptr as *const acpi_madt_eio_pic)),
                ACPI_MADT_ENTRY_TYPE_MSI_PIC => MsiPic(&*(self.ptr as *const acpi_madt_msi_pic)),
                ACPI_MADT_ENTRY_TYPE_BIO_PIC => BioPic(&*(self.ptr as *const acpi_madt_bio_pic)),
                ACPI_MADT_ENTRY_TYPE_LPC_PIC => LpcPic(&*(self.ptr as *const acpi_madt_lpc_pic)),
                ACPI_MADT_ENTRY_TYPE_RINTC => Rintc(&*(self.ptr as *const acpi_madt_rintc)),
                ACPI_MADT_ENTRY_TYPE_IMSIC => Imsic(&*(self.ptr as *const acpi_madt_imsic)),
                ACPI_MADT_ENTRY_TYPE_APLIC => Aplic(&*(self.ptr as *const acpi_madt_aplic)),
                ACPI_MADT_ENTRY_TYPE_PLIC => Plic(&*(self.ptr as *const acpi_madt_plic)),
                _ => Unknown(&*core::ptr::slice_from_raw_parts(
                    self.ptr as *mut u8,
                    hdr.length as usize,
                )),
            };

            self.ptr = self.ptr.byte_add(hdr.length as usize);
            self.remaining -= hdr.length as usize;

            Some(res)
        }
    }
}
