// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{any::Any, fmt::Display, mem::offset_of, sync::atomic::Ordering};

use alloc::sync::Arc;
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    cpu::timer::time_us,
    dev2::{
        Device, DeviceBase,
        bus::{
            Bus, BusResv,
            ata::Command,
            pci::{PciBus, addr::PciIrq, bar::BarType, classcode},
            soc::{MmioMapping, MmioStruct},
        },
        class::atactl::AtaCtlDevice,
        driver::Driver,
        registry,
    },
    device_get_trait_vtable,
    driver2::sata::port::Port,
    kernel::{sched::thread_sleep, sync::spinlock::RawSpinlock},
    mem::dma::DmaTarget,
    register_kmodule,
};

mod fis;
mod hms;
mod port;
mod reg;

/// SATA AHCI controller.
pub struct SataAhciCtl {
    base: DeviceBase,
    bus: BusResv<PciBus>,
    reg: MmioStruct<reg::Ctrl>,
    port: [Option<port::Port>; 32],
    /// Spinlock that guards the control registers.
    reg_lock: RawSpinlock,
}
unsafe impl Sync for SataAhciCtl {}

impl Drop for SataAhciCtl {
    fn drop(&mut self) {
        unsafe {
            self.bus
                .take_unchecked()
                .uninstall_pci_irq(PciIrq::IntA, self)
        };
    }
}

impl SataAhciCtl {
    pub fn new(base: DeviceBase, bus: BusResv<PciBus>) -> EResult<Arc<Self>> {
        // Map BAR no. 5.
        let info = unsafe { bus.take()?.bar_info() }?;
        let bar5 = info[5].unwrap();
        assert!(bar5.type_ != BarType::IO);
        let reg = unsafe {
            MmioStruct::<reg::Ctrl>::new_unsafe_size(
                MmioMapping::new(bar5.cpu_paddr, bar5.size, true, false)?,
                offset_of!(reg::Ctrl, port) + size_of::<reg::Port>(),
            )?
        };

        let supports_ss = reg.ghc.cap.read(reg::HostCaps::supports_ss) != 0;

        // Perform BIOS/OS handoff (if required).
        if reg.ghc.cap2.read(reg::HostCapsExt::supports_bios_handoff) != 0 {
            let lim = time_us() + 2000000;
            reg.ghc.bohc.modify(reg::HostBOHC::os_owned.val(1));
            while reg.ghc.bohc.read(reg::HostBOHC::bios_owned) != 0
                || reg.ghc.bohc.read(reg::HostBOHC::bios_busy) != 0
            {
                if time_us() > lim {
                    logkf!(LogLevel::Error, "Failed to take HBA ownership");
                    return Err(Errno::ENAVAIL);
                }
                let _ = thread_sleep(10000);
            }
        }

        // Reset the HBA.
        reg.ghc.ghc.modify(reg::HostCtrl::hba_reset.val(1));
        let lim = time_us() + 1000000;
        while reg.ghc.ghc.read(reg::HostCtrl::hba_reset) != 0 {
            if time_us() > lim {
                logkf!(LogLevel::Error, "Failed to reset HBA");
                return Err(Errno::ENAVAIL);
            }
            let _ = thread_sleep(10000);
        }

        // Switch to AHCI mode if it wasn't already in that mode.
        reg.ghc.ghc.modify(reg::HostCtrl::ahci_en.val(1));
        let cmdlist_max = reg.ghc.cap.read(reg::HostCaps::n_cmd_slots);

        // Reset all ports.
        // TODO: If cold presence detection isn't supported,
        // ports with no devices connected should be treated as not implemented.
        let ports_impl = reg.ghc.ports_impl.get();
        let mut port = [const { None }; 32];
        for i in 0..32 {
            if (ports_impl >> i) & 1 != 0 {
                // Just in case firmware forgot to properly stop this port.
                reg.port[i].irq_enable.set(0);

                match Port::new(i as u8, supports_ss, &reg.port[i], cmdlist_max as usize) {
                    Ok(it) => port[i] = Some(it),
                    Err(err) => logkf!(
                        LogLevel::Warning,
                        "{}: AHCI port {} setup failed: {}",
                        bus,
                        i,
                        err
                    ),
                }
            }
        }

        // Create the device instance.
        let this = Arc::try_new(Self {
            base,
            bus,
            reg,
            port,
            reg_lock: RawSpinlock::new(),
        })?;

        // Start the driver threads.
        let mut err: EResult<()> = Ok(());
        for i in 0..32 {
            if let Some(ref port) = this.port[i] {
                if let Err(x) = port.start(this.clone()) {
                    logkf!(
                        LogLevel::Error,
                        "{}: AHCI port {} start failed: {}",
                        &this.bus,
                        i,
                        x
                    );
                    err = Err(x);
                    break;
                }
            }
        }

        // If starting any of the threads failed, stop all other threads.
        if let Err(err) = err {
            for i in 0..32 {
                if let Some(ref port) = this.port[i] {
                    port.stop();
                }
            }
            return Err(err);
        }

        // If all succeeded, install and enable interrupts.
        unsafe {
            this.bus
                .take()?
                .install_pci_irq(PciIrq::IntA, &*this as &dyn Device)?;
            this.reg.ghc.ghc.modify(reg::HostCtrl::irq_en::SET);
        };

        Ok(this)
    }
}

impl Display for SataAhciCtl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.bus, f)
    }
}

impl Device for SataAhciCtl {
    fn base(&self) -> &DeviceBase {
        &self.base
    }

    fn interrupt(&self, _id: u128) -> bool {
        let _guard = self.reg_lock.lock();
        let reg = &self.reg.ghc;

        let orig_stat = reg.irq_status.get();
        let mut stat = orig_stat;
        while stat != 0 {
            let index = stat.trailing_zeros() as usize;
            if let Some(port) = &self.port[index] {
                port.interrupt(self);
            } else {
                panic!("Interrupt on non-existant port {}", index);
            }
            stat &= !(1 << index);
        }
        reg.irq_status.set(orig_stat);

        true
    }

    device_get_trait_vtable!(AtaCtlDevice);
}

impl AtaCtlDevice for SataAhciCtl {
    fn ata_cmd(
        &self,
        port: u32,
        cmd: Command,
        ctrl: u8,
        sec_count: u16,
        feature: u16,
        lba: u64,
        data_offset: u64,
        data_length: u64,
        data: Option<&dyn DmaTarget>,
    ) -> EResult<()> {
        self.port
            .get(port as usize)
            .ok_or(Errno::ENODEV)?
            .as_ref()
            .ok_or(Errno::ENODEV)?
            .ata_cmd(
                cmd,
                ctrl,
                sec_count,
                feature,
                lba,
                data_offset,
                data_length,
                data,
            )
    }
}

pub struct SataDriver;

impl Driver for SataDriver {
    fn name(&self) -> &str {
        "sata-ahci"
    }

    fn match_(&self, bus: &dyn Bus) -> bool {
        let Some(bus) = (bus as &dyn Any).downcast_ref::<PciBus>() else {
            return false;
        };

        bus.classcode == classcode::storage::sata::ahci
    }

    unsafe fn probe(&self, bus: BusResv<dyn Bus>) -> EResult<Arc<dyn Device>> {
        let bus = bus.downcast().unwrap();
        let dev = SataAhciCtl::new(DeviceBase::new(), bus)?;
        Ok(dev)
    }
}

register_kmodule!("sata-ahci", || registry::register_driver(&SataDriver));
