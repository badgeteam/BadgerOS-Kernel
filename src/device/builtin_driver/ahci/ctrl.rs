use core::mem::MaybeUninit;

use super::reg;
use alloc::{boxed::Box, vec};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

use crate::{
    LogLevel, base_driver_struct,
    bindings::{
        device::{
            BaseDriver, Device, DeviceInfo, DeviceInfoView, HasBaseDevice,
            addr::{AhciAddr, DevAddr, PciAddr},
            class::pcictl::{MappedBar, PciCtlDevice},
        },
        error::{EResult, Errno},
        raw::{
            driver_t, irqno_t, pci_bclass_t_PCI_BCLASS_STORAGE,
            pci_progif_storage_sata_t_PCI_PROGIF_STORAGE_SATA_AHCI,
            pci_subclass_storage_t_PCI_SUBCLASS_STORAGE_SATA, pcie_hdr_com_t,
        },
        time_us,
    },
    boot::init::INIT_BLOCK_THREADS,
    kernel::sched::{Thread, thread_sleep},
    logk,
    mem::vmm::kernel_mm,
};

/// Match an AHCI controller.
fn ahci_match(info: DeviceInfoView<'_>) -> bool {
    // Ensure that parent is a PCI controller.
    if info.parent().is_none() {
        return false;
    }
    let parent = info.parent().unwrap().as_pcictl();
    if parent.is_none() {
        return false;
    }
    let parent = parent.unwrap();

    // Check that function 0 is AHCI.
    if info.addrs().len() == 0 {
        return false;
    }
    let addr: DevAddr = info.addrs()[0].into();
    let addr = if let DevAddr::Pci(addr) = addr {
        addr
    } else {
        return false;
    };

    // Read the PCIe header so we can tell the type of device this is.
    let mut header = MaybeUninit::<pcie_hdr_com_t>::uninit();
    unsafe { parent.dev_cam_read(addr, 0, &mut header) };
    let header = unsafe { header.assume_init() };

    // Check that this is a SATA AHCI controller.
    if header.classcode.baseclass != pci_bclass_t_PCI_BCLASS_STORAGE
        || header.classcode.subclass != pci_subclass_storage_t_PCI_SUBCLASS_STORAGE_SATA
        || header.classcode.progif != pci_progif_storage_sata_t_PCI_PROGIF_STORAGE_SATA_AHCI
    {
        return false;
    }

    true
}

/// The AHCI controller driver.
pub(super) struct AhciDriver {
    pub(super) parent: PciCtlDevice,
    pub(super) device: Device,
    pub(super) bar_mapping: MappedBar,
    pub(super) mmio: &'static reg::Host,
    pub(super) ports_mmio: &'static [reg::Port; 32],
}

unsafe impl Sync for AhciDriver {}

impl AhciDriver {
    pub fn new(device: Device) -> EResult<Box<Self>> {
        // Get the parent device, which should be a PCI controller.
        let parent = device
            .info()
            .parent()
            .and_then(|x| x.as_pcictl())
            .ok_or(Errno::EINVAL)?;

        let addr: PciAddr = unsafe { device.info().addrs()[0].__bindgen_anon_1.pci }.into();
        logkf!(
            LogLevel::Info,
            "Detected SATA AHCI controller at {:02x}:{:02x}.{:o}",
            addr.bus,
            addr.dev,
            addr.func
        );

        // Map BAR no. 5.
        let bar_info = unsafe { parent.bar_info(addr) };
        logkf!(
            LogLevel::Debug,
            "Mapping AHCI controller BAR {:#x?}",
            &bar_info[5]
        );
        let bar_mapping = unsafe { parent.bar_map(bar_info[5]) }?;
        logkf!(
            LogLevel::Debug,
            "BAR mapped at {:#x?}",
            bar_mapping.as_ptr() as usize
        );
        let mmio = unsafe { &*(bar_mapping.as_ptr() as *const reg::Host) };
        let ports_mmio =
            unsafe { &*(bar_mapping.as_ptr().wrapping_add(0x100) as *const [reg::Port; 32]) };

        // Perform BIOS/OS handoff (if required).
        if mmio.cap2.read(reg::HostCapsExt::supports_bios_handoff) != 0 {
            let lim = time_us() + 2000000;
            mmio.bohc.modify(reg::HostBOHC::os_owned.val(1));
            while mmio.bohc.read(reg::HostBOHC::bios_owned) != 0
                || mmio.bohc.read(reg::HostBOHC::bios_busy) != 0
            {
                if time_us() > lim {
                    logk(LogLevel::Warning, "Failed to take HBA ownership");
                    return Err(Errno::ENAVAIL);
                }
                let _ = thread_sleep(10000);
            }
        }

        // Reset the HBA.
        mmio.ghc.modify(reg::HostCtrl::hba_reset.val(1));
        let lim = time_us() + 1000000;
        while mmio.ghc.read(reg::HostCtrl::hba_reset) != 0 {
            if time_us() > lim {
                logk(LogLevel::Warning, "Failed to reset HBA");
                return Err(Errno::ENAVAIL);
            }
            let _ = thread_sleep(10000);
        }

        // Switch to AHCI mode if it wasn't already in that mode.
        mmio.ghc.modify(reg::HostCtrl::ahci_en.val(1));

        // Reset all ports.
        let ports_impl = mmio.ports_impl.get();
        for i in 0..32 {
            if (ports_impl >> i) & 1 != 0 {
                ports_mmio[i].irq_enable.set(0u32);
                ports_mmio[i].irq_status.set(ports_mmio[i].irq_status.get());
            }
        }

        // Detect drives in ports.
        let mut drives = vec![];
        for i in 0..32 {
            if (ports_impl >> i) & 1 != 0
                && ports_mmio[i].sstatus.read(reg::PortSStatus::detect) != 0
            {
                logkf!(LogLevel::Info, "Found drive in slot {}", i);
                let res = Device::add(DeviceInfo {
                    parent: Some(device.clone()),
                    irq_parent: Some(device.clone()),
                    addrs: vec![DevAddr::Ahci(AhciAddr {
                        port: i as u8,
                        pmul: None,
                    })],
                    phandle: None,
                });
                match res {
                    Err(x) => logkf!(LogLevel::Error, "Could not add drive {}: {}", i, x),
                    Ok(x) => {
                        drives.push(x);
                    }
                }
            }
        }
        let discover_thread = Thread::new(
            move || {
                for drive in drives {
                    drive.activate();
                }
            },
            None,
            Some(format!("AHCI {} discover", device.id())),
        )?;
        INIT_BLOCK_THREADS.unintr_lock().push(discover_thread);

        // Enable interrupts.
        unsafe {
            // Configure the PCI controller to use INTA.
            parent.dev_cam_write(addr.into(), 0x3D, &1u8);
            // Enable PCI INTA.
            device.cascase_enable_irq_out(1).unwrap();
            // Enable AHCI interrupts globally.
            mmio.ghc.modify(reg::HostCtrl::irq_en.val(1));
        }

        Ok(Box::new(Self {
            parent,
            device,
            bar_mapping,
            mmio,
            ports_mmio,
        }))
    }
}

impl BaseDriver for AhciDriver {
    fn remove(&mut self) {
        todo!()
    }

    fn interrupt(&self, _: irqno_t) -> bool {
        let port_irqs = self.mmio.irq_status.get();
        let mut handled = 0u32;
        while (port_irqs ^ handled) != 0 {
            let port = (port_irqs ^ handled).trailing_zeros();
            handled |= 1u32 << port;
            assert!(
                unsafe { self.device.forward_interrupt(port) },
                "SATA drive did not handle its interrupt"
            );
        }
        true
    }
}

/// The AHCI controller driver struct.
pub(super) static AHCI_DRIVER: driver_t =
    base_driver_struct!(AhciDriver, ahci_match, AhciDriver::new);
