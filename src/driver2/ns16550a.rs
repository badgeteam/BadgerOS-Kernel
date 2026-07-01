// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::any::Any;

use alloc::sync::Arc;
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_structs,
    registers::ReadWrite,
};

use crate::{
    badgelib::{
        fifo::{BlockingFifo, Fifo},
        irq::IrqGuard,
    },
    bindings::error::EResult,
    dev2::{
        Device, DeviceBase,
        bus::{
            Bus,
            soc::{MmioStruct, SocBus},
        },
        class::char::CharDevice,
        driver::Driver,
    },
    device_get_trait_vtable,
    kernel::sync::{spinlock::Spinlock, waitlist::Waitlist},
    process::{
        uapi::termios,
        usercopy::{UserSlice, UserSliceMut},
    },
};

/// Enable for receive data available IRQ.
const IER_RX_AVL: u8 = 0x01;
/// Enable for transmit data empty IRQ.
const IER_TX_EMPTY: u8 = 0x02;
/// Enable for receive line status IRQ.
const IER_RX_LINE: u8 = 0x04;
/// Enable for modem status interrupt.
const IER_MODEM: u8 = 0x08;

/// Interrupt pending.
const IIR_PENDING: u8 = 0x01;
/// Pending interrupt ID bitmask.
const IIR_ID_MASK: u8 = 0x0e;
/// Pending interrupt ID bit exponent.
const IIR_ID_POS: u32 = 1;
/// FIFOs enabled bitmask.
const IIR_FIFO_MASK: u8 = 0xc0;
/// FIFOs enabled bit exponent.
const IIR_FIFO_POS: u32 = 6;

/// FIFO enable.
const FCR_FIFO_EN: u8 = 0x01;
/// Receive FIFO clear.
const FCR_RXFIFO_CLEAR: u8 = 0x02;
/// Transmit FIFO clear.
const FCR_TXFIFO_CLEAR: u8 = 0x04;
/// TODO: What is this? DMA mode select.
const FCR_DMA_MODE: u8 = 0x08;
/// Receiver trigger bitmask.
const FCR_RXTRIG_MASK: u8 = 0xc0;
/// Receiver trigger bit exponent.
const FCR_RXTRIG_POS: u32 = 6;

/// Word length select bitmask.
const LCR_WORDLEN_MASK: u8 = 0x03;
/// Word length select bit exponent.
const LCR_WORDLEN_POS: u32 = 0;
/// Number of stop bits.
const LCR_STOPBITS: u8 = 0x04;
/// Parity enable.
const LCR_PARITY_EN: u8 = 0x08;
/// Even parity select.
const LCR_PARITY_EVEN: u8 = 0x10;
/// TODO: What is this?
const LCR_PARITY_STICK: u8 = 0x20;
/// TODO: What is this?
const LCR_SET_BREAK: u8 = 0x40;
/// Divisor latch access bit.
const LCR_DLAB: u8 = 0x80;

/// Data terminal ready.
const MCR_DTR: u8 = 0x01;
/// Request to send.
const MCR_RTS: u8 = 0x02;
/// Out 1.
const MCR_OUT1: u8 = 0x04;
/// Out 2.
const MCR_OUT2: u8 = 0x08;
/// Loop.
const MCR_LOOP: u8 = 0x10;

/// Receive data is available.
const LSR_DATA_READY: u8 = 0x01;
/// Overrun error.
const LSR_OVERRUN_ERR: u8 = 0x02;
/// Parity error.
const LSR_PARITY_ERR: u8 = 0x04;
/// Framing error.
const LSR_FRAME_ERR: u8 = 0x08;
/// Break interrupt.
const LSR_BREAK_IRQ: u8 = 0x10;
/// TODO: What is this? Transmitter holding register.
const LSR_TX_HOLD_REG: u8 = 0x20;
/// Transmitter ir ready for data.
const LSR_TX_EMPTY: u8 = 0x40;
/// Error in receiver FIFO.
const LSR_RX_FIFO_ERR: u8 = 0x80;

/// Delta clear to send.
const MSR_DELTA_CTS: u8 = 0x01;
/// Delta set ready.
const MSR_DELTA_SET_READY: u8 = 0x02;
/// Trailing edge ring indicator.
const MSR_TRAILING_RING: u8 = 0x04;
/// Delta data carrier detect.
const MSR_DELTA_DCD: u8 = 0x08;
/// Clear to send.
const MSR_CTS: u8 = 0x10;
/// Data set ready.
const MSR_SET_READY: u8 = 0x20;
/// Ring indicator.
const MSR_RING: u8 = 0x40;
/// Data carrier detect.
const MSR_DCD: u8 = 0x80;

register_structs! {
    /// Definition of NS16550A UART controller registers.
    Ns16550a {
        /// FIFO read/write port.
        (0 => fifo:     ReadWrite<u8>),
        /// Interrupt enable register.
        (1 => ier:      ReadWrite<u8>),
        /// Interrupt identification register / FIFO control register.
        (2 => iid_fcr:  ReadWrite<u8>),
        /// Line control register.
        (3 => lcr:      ReadWrite<u8>),
        /// Modem control register.
        (4 => mcr:      ReadWrite<u8>),
        /// Line status register.
        (5 => lsr:      ReadWrite<u8>),
        /// Modem status register.
        (6 => msr:      ReadWrite<u8>),
        /// Scratch register.
        (7 => _resvd0:  u8),
        /// End of structure.
        (8 => @END),
    }
}

/// Driver for an NS16550A-compatible device.
pub struct Ns16550aDevice {
    base: DeviceBase,
    bus: Arc<SocBus>,
    regs: Spinlock<MmioStruct<Ns16550a>>,
    txfifo: BlockingFifo,
    rxfifo: BlockingFifo,
    attr: Spinlock<termios::termios>,
}

impl Ns16550aDevice {
    /// # Safety
    /// The caller must guarantee that the bus points to a valid NS16550A-compatible device.
    pub unsafe fn new(base: DeviceBase, bus: Arc<SocBus>) -> EResult<Arc<Self>> {
        let regs = unsafe { MmioStruct::new(bus.map(0)?) }?;

        let this = Arc::try_new(Self {
            base,
            bus: bus.clone(),
            regs: Spinlock::new(regs),
            txfifo: BlockingFifo::new(Fifo::DEFAULT_SIZE)?,
            rxfifo: BlockingFifo::new(Fifo::DEFAULT_SIZE)?,
            attr: Spinlock::new(Default::default()),
        })?;
        // SAFETY: Cleaned up in `impl Drop`.
        unsafe { bus.install_irq(0, this.as_ref())? };
        this.check_fifos();

        Ok(this)
    }

    fn check_fifos(&self) {
        let regs = self.regs.lock();
        let attr = self.attr.lock();

        // Read all available receive data.
        while regs.lsr.get() & LSR_DATA_READY != 0 {
            // FIFO overflow is ignored.
            let mut c = regs.fifo.get();
            if attr.c_iflag & termios::ICRNL != 0 && c == b'\r' {
                c = b'\n';
            } else if attr.c_iflag & termios::INLCR != 0 && c == b'\n' {
                c = b'\r';
            }
            if attr.c_lflag & termios::ECHO != 0 {
                regs.fifo.set(c);
            }
            let _ = self.rxfifo.writek(&[c], true);
        }

        // Write all pending send data that will fit.
        while regs.lsr.get() & LSR_TX_EMPTY != 0 {
            let mut tmp = [0u8];
            // This readk can't fail because it is non-blocking on kernel memory.
            if self.txfifo.readk(&mut tmp, true).unwrap() == 0 {
                break;
            }
            let mut c = tmp[0];
            if attr.c_oflag & termios::OCRNL != 0 && c == b'\r' {
                c = b'\n';
            } else if attr.c_oflag & termios::ONLCR != 0 && c == b'\n' {
                c = b'\r';
            }
            regs.fifo.set(c);
        }

        // We only want the interrupt for transmit data empty if we have anything in the FIFO.
        if self.txfifo.read_avl() > 0 {
            regs.ier.set(IER_RX_AVL | IER_TX_EMPTY);
        } else {
            regs.ier.set(IER_RX_AVL);
        }
    }
}

impl CharDevice for Ns16550aDevice {
    fn read_waitlist(&self) -> Option<&Waitlist> {
        Some(self.rxfifo.read_waitlist())
    }

    fn write_waitlist(&self) -> Option<&Waitlist> {
        Some(self.txfifo.write_waitlist())
    }

    fn poll(&self, read: bool, write: bool) -> bool {
        (read && self.rxfifo.read_avl() > 0) || (write && self.txfifo.write_avl() > 0)
    }

    fn read_raw(&self, rdata: UserSliceMut<u8>, nonblock: bool) -> EResult<usize> {
        self.rxfifo.read(rdata, nonblock)
    }

    fn write_raw(&self, wdata: UserSlice<u8>, nonblock: bool) -> EResult<usize> {
        let res = self.txfifo.write(wdata, nonblock);
        self.check_fifos();
        res
    }
}

impl Drop for Ns16550aDevice {
    fn drop(&mut self) {
        // Cleaning up the `Bus::install_irq` from `Self::new`.
        unsafe { self.bus.uninstall_irq(0, self) };
    }
}

impl Device for Ns16550aDevice {
    fn base(&self) -> &DeviceBase {
        &self.base
    }

    fn interrupt(&self, _id: u128) -> bool {
        self.check_fifos();
        true
    }

    device_get_trait_vtable!(CharDevice);
}

/// The NS16550A driver, registered into the dev2 driver table.
pub struct Ns16550aDriver;

impl Driver for Ns16550aDriver {
    fn name(&self) -> &str {
        "ns16550a"
    }

    fn match_(&self, bus: &dyn Bus) -> bool {
        if (bus as &dyn Any).downcast_ref::<SocBus>().is_none() {
            return false;
        }
        let Some(node) = bus.dtb_node() else {
            return false;
        };
        node.is_compatible_any(&["ns16550a"])
    }

    unsafe fn probe(&self, bus: Arc<dyn Bus>) -> EResult<Arc<dyn Device>> {
        let bus = Arc::downcast::<SocBus>(bus).unwrap();
        // SAFETY: The bus was matched as an NS16550A-compatible device.
        let device = unsafe { Ns16550aDevice::new(DeviceBase::new(), bus.clone())? };
        bus.claim(Arc::<Ns16550aDevice>::downgrade(&device))?;
        Ok(device)
    }
}
