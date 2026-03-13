use crate::cpu;

/// Guard that disable interrupts momentarily.
pub struct IrqGuard {
    was_enabled: bool,
}

impl IrqGuard {
    pub fn new() -> Self {
        IrqGuard {
            was_enabled: unsafe { cpu::irq::disable() },
        }
    }
}

impl Drop for IrqGuard {
    fn drop(&mut self) {
        unsafe { cpu::irq::enable_if(self.was_enabled) };
    }
}
