// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_structs,
    registers::ReadWrite,
};

use crate::{
    LogLevel,
    badgelib::{
        fifo::{BlockingFifo, Fifo},
        irq::IrqGuard,
    },
    bindings::{
        device::{BaseDriver, Device, DeviceInfoView, HasBaseDevice, class::char::CharDriver},
        error::{EResult, Errno},
        raw::{dev_atype_t_DEV_ATYPE_MMIO, driver_char_t, irqno_t},
        spinlock::Spinlock,
    },
    char_driver_struct,
    process::usercopy::{UserSlice, UserSliceMut},
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

struct Ns16550aDriver {
    device: Device,
    txfifo: BlockingFifo,
    rxfifo: BlockingFifo,
    regs: Spinlock<&'static Ns16550a>,
}

impl Ns16550aDriver {
    /// Match an NS16550A.
    fn match_(info: DeviceInfoView<'_>) -> bool {
        #[cfg(feature = "dtb")]
        if info.dtb_match(&["ns16550a"]) {
            return true;
        }
        false
    }

    pub fn new(device: Device) -> EResult<Box<Self>> {
        let info = device.info();
        if info.addrs().len() != 1 || info.addrs()[0].type_ != dev_atype_t_DEV_ATYPE_MMIO {
            logkf!(LogLevel::Error, "Invalid addresses for NS16550A");
            return Err(Errno::EIO);
        }
        let regs =
            unsafe { &*(device.info().addrs()[0].__bindgen_anon_1.mmio.vaddr as *const Ns16550a) };

        // Reset the UART.
        regs.iid_fcr.set(0);
        regs.ier.set(0);
        regs.iid_fcr
            .set(FCR_FIFO_EN | FCR_RXFIFO_CLEAR | FCR_TXFIFO_CLEAR);

        Ok(Box::try_new(Self {
            device,
            txfifo: BlockingFifo::new(Fifo::DEFAULT_SIZE)?,
            rxfifo: BlockingFifo::new(Fifo::DEFAULT_SIZE)?,
            regs: Spinlock::new(regs),
        })?)
    }
}

impl BaseDriver for Ns16550aDriver {
    fn post_add(&self) {
        // Start this interrupt-driven driver properly.
        // Cannot be done earlier because interrupts may not be touched during driver init.
        let _guard = IrqGuard::new();
        self.interrupt(0);
        unsafe { self.device.cascase_enable_irq_out(0).unwrap() };
    }

    fn interrupt(&self, _irq: irqno_t) -> bool {
        let regs = self.regs.lock();

        // Read all available receive data.
        while regs.lsr.get() & LSR_DATA_READY != 0 {
            // FIFO overflow is ignored.
            let _ = self.rxfifo.writek(&[regs.fifo.get()], true);
        }

        // Write all pending send data that will fit.
        while regs.lsr.get() & LSR_TX_EMPTY != 0 {
            let mut tmp = [0u8];
            // This readk can't fail because it is non-blocking on kernel memory.
            if self.txfifo.readk(&mut tmp, true).unwrap() == 0 {
                break;
            }
            regs.fifo.set(tmp[0]);
        }

        // We only want the interrupt for transmit data empty if we have anything in the FIFO.
        if self.txfifo.read_avl() > 0 {
            regs.ier.set(IER_RX_AVL | IER_TX_EMPTY);
        } else {
            regs.ier.set(IER_RX_AVL);
        }

        true
    }
}

impl CharDriver for Ns16550aDriver {
    fn read(&self, rdata: UserSliceMut<'_, u8>, nonblock: bool) -> EResult<usize> {
        Ok(self.rxfifo.read(rdata, nonblock)?)
    }

    fn write(&self, wdata: UserSlice<'_, u8>, nonblock: bool) -> EResult<usize> {
        let wcount = self.txfifo.write(wdata, nonblock);

        let _guard = IrqGuard::new();
        self.interrupt(0);

        Ok(wcount?)
    }
}

/// The AHCI controller driver struct.
pub(super) static NS16550A_DRIVER: driver_char_t =
    char_driver_struct!(Ns16550aDriver, Ns16550aDriver::match_, Ns16550aDriver::new);
