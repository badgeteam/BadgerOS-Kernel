use crate::arch::{Arch, except::ArchExcept};

/// Guard that disable interrupts momentarily.
pub struct IrqGuard {
    was_enabled: bool,
}

impl IrqGuard {
    pub fn new() -> Self {
        IrqGuard {
            was_enabled: Arch::get_disable_irq(),
        }
    }
}

impl Drop for IrqGuard {
    fn drop(&mut self) {
        Arch::enable_irq_if(self.was_enabled);
    }
}
