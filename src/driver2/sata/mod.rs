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
            pci::{PciBus, bar::BarType, classcode},
            soc::{MmioMapping, MmioStruct},
        },
        driver::Driver,
    },
    device_get_trait_vtable,
    driver2::sata::port::Port,
    kernel::{
        sched::{Thread, thread_sleep},
        sync::spinlock::RawSpinlock,
    },
};

mod fis;
mod hms;
mod port;
mod reg;

/// SATA AHCI controller.
pub struct SataAhciCtrl {
    base: DeviceBase,
    bus: BusResv<PciBus>,
    reg: MmioStruct<reg::Ctrl>,
    port: [Option<port::Port>; 32],
    /// Spinlock that guards the control registers.
    reg_lock: RawSpinlock,
}
unsafe impl Sync for SataAhciCtrl {}

impl SataAhciCtrl {
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

        // Reset all ports.
        let ports_impl = reg.ghc.ports_impl.get();
        let mut port = [const { None }; 32];
        for i in 0..32 {
            if (ports_impl >> i) & 1 != 0 {
                reg.port[i].irq_enable.set(0u32);
                reg.port[i].irq_status.set(reg.port[i].irq_status.get());

                match Port::new(i as u8, &reg.port[i]) {
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

        Ok(this)
    }
}

impl Display for SataAhciCtrl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.bus, f)
    }
}

impl Device for SataAhciCtrl {
    fn base(&self) -> &DeviceBase {
        &self.base
    }

    fn interrupt(&self, _id: u128) -> bool {
        let _guard = self.reg_lock.lock();
        let reg = &self.reg.ghc;

        let mut stat = reg.irq_status.get();
        while stat != 0 {
            let index = stat.trailing_zeros() as usize;
            if let Some(port) = &self.port[index] {
                port.interrupt(self);
            } else {
                panic!("Interrupt on non-existant port {}", index);
            }
            stat &= !(1 << index);
        }
        reg.irq_status.set(stat);

        true
    }

    device_get_trait_vtable!();
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
        let dev = SataAhciCtrl::new(DeviceBase::new(), bus)?;
        Ok(dev)
    }
}
