// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::arch::asm;

/// SBI-call errors.
#[repr(isize)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SbiError {
    Failed = 1,
    NotSupported = 2,
    InvalidParam = 3,
    Denied = 4,
    InvalidAddress = 5,
    AlreadyAvailable = 6,
    AlreadyStarted = 7,
    AlreadyStopped = 8,
    NoShmem = 9,
}

impl From<isize> for SbiError {
    fn from(value: isize) -> Self {
        use SbiError::*;
        match value {
            1 => Failed,
            2 => NotSupported,
            3 => InvalidParam,
            4 => Denied,
            5 => InvalidAddress,
            6 => AlreadyAvailable,
            7 => AlreadyStarted,
            8 => AlreadyStopped,
            9 => NoShmem,
            _ => Failed,
        }
    }
}

/// SBI-call result.
pub type SbiResult<T = isize> = Result<T, SbiError>;

macro_rules! sbi_call {
    ($fid: expr, $eid: expr, $a0: expr, $a1: expr, $a2: expr, $a3: expr, $a4: expr, $a5: expr $(,)?) => {
        unsafe {
            let mut a0: isize = $a0;
            let mut a1: isize = $a1;
            let a2: isize = $a2;
            let a3: isize = $a3;
            let a4: isize = $a4;
            let a5: isize = $a5;
            let a6: isize = $fid;
            let a7: isize = $eid;
            asm!(
                "ecall",
                inout("a0") a0,
                inout("a1") a1,
                in("a2") a2,
                in("a3") a3,
                in("a4") a4,
                in("a5") a5,
                in("a6") a6,
                in("a7") a7,
            );
            if a0 == 0 {
                Ok(a1)
            } else {
                Err(SbiError::from(-a0))
            }
        }
    };
    ($fid: expr, $eid: expr, $a0: expr, $a1: expr, $a2: expr, $a3: expr, $a4: expr $(,)?) => {
        unsafe {
            let mut a0: isize = $a0;
            let mut a1: isize = $a1;
            let a2: isize = $a2;
            let a3: isize = $a3;
            let a4: isize = $a4;
            let a6: isize = $fid;
            let a7: isize = $eid;
            asm!(
                "ecall",
                inout("a0") a0,
                inout("a1") a1,
                in("a2") a2,
                in("a3") a3,
                in("a4") a4,
                in("a6") a6,
                in("a7") a7,
            );
            if a0 == 0 {
                Ok(a1)
            } else {
                Err(SbiError::from(-a0))
            }
        }
    };
    ($fid: expr, $eid: expr, $a0: expr, $a1: expr, $a2: expr, $a3: expr $(,)?) => {
        unsafe {
            let mut a0: isize = $a0;
            let mut a1: isize = $a1;
            let a2: isize = $a2;
            let a3: isize = $a3;
            let a6: isize = $fid;
            let a7: isize = $eid;
            asm!(
                "ecall",
                inout("a0") a0,
                inout("a1") a1,
                in("a2") a2,
                in("a3") a3,
                in("a6") a6,
                in("a7") a7,
            );
            if a0 == 0 {
                Ok(a1)
            } else {
                Err(SbiError::from(-a0))
            }
        }
    };
    ($fid: expr, $eid: expr, $a0: expr, $a1: expr, $a2: expr $(,)?) => {
        unsafe {
            let mut a0: isize = $a0;
            let mut a1: isize = $a1;
            let a2: isize = $a2;
            let a6: isize = $fid;
            let a7: isize = $eid;
            asm!(
                "ecall",
                inout("a0") a0,
                inout("a1") a1,
                in("a2") a2,
                in("a6") a6,
                in("a7") a7,
            );
            if a0 == 0 {
                Ok(a1)
            } else {
                Err(SbiError::from(-a0))
            }
        }
    };
    ($fid: expr, $eid: expr, $a0: expr, $a1: expr $(,)?) => {
        unsafe {
            let mut a0: isize = $a0;
            let mut a1: isize = $a1;
            let a6: isize = $fid;
            let a7: isize = $eid;
            asm!(
                "ecall",
                inout("a0") a0,
                inout("a1") a1,
                in("a6") a6,
                in("a7") a7,
            );
            if a0 == 0 {
                Ok(a1)
            } else {
                Err(SbiError::from(-a0))
            }
        }
    };
    ($fid: expr, $eid: expr, $a0: expr $(,)?) => {
        unsafe {
            let mut a0: isize = $a0;
            let a1: isize;
            let a6: isize = $fid;
            let a7: isize = $eid;
            asm!(
                "ecall",
                inout("a0") a0,
                out("a1") a1,
                in("a6") a6,
                in("a7") a7,
            );
            if a0 == 0 {
                Ok(a1)
            } else {
                Err(SbiError::from(-a0))
            }
        }
    };
    ($fid: expr, $eid: expr $(,)?) => {
        unsafe {
            let a0: isize;
            let a1: isize;
            let a6: isize = $fid;
            let a7: isize = $eid;
            asm!(
                "ecall",
                out("a0") a0,
                out("a1") a1,
                in("a6") a6,
                in("a7") a7,
            );
            if a0 == 0 {
                Ok(a1)
            } else {
                Err(SbiError::from(-a0))
            }
        }
    };
}

const BASE_EID: isize = 0x10;

pub fn get_spec_version() -> SbiResult {
    sbi_call!(0, BASE_EID)
}

pub fn get_impl_id() -> SbiResult {
    sbi_call!(1, BASE_EID)
}

pub fn get_impl_version() -> SbiResult {
    sbi_call!(2, BASE_EID)
}

pub fn probe_extension(eid: isize) -> SbiResult {
    sbi_call!(3, BASE_EID, eid)
}

pub fn get_mvendorid() -> SbiResult {
    sbi_call!(4, BASE_EID)
}

pub fn get_marchid() -> SbiResult {
    sbi_call!(5, BASE_EID)
}

pub fn get_mimpid() -> SbiResult {
    sbi_call!(6, BASE_EID)
}

/// SBI legacy extension.
pub mod legacy {
    use core::arch::asm;

    /// Set a timer to fire `delta_ticks` timer ticks in the future.
    #[cfg(target_arch = "riscv32")]
    pub fn set_timer(delta_ticks: u64) -> Result<(), ()> {
        unsafe {
            let mut a0 = delta_ticks as isize;
            let a1 = (delta_ticks >> 32) as isize;
            let a7 = 0x00isize;
            asm!(
                "ecall",
                inout("a0") a0,
                in("a1") a1,
                in("a7") a7
            );
            if a0 == 0 { Ok(()) } else { Err(()) }
        }
    }

    /// Set a timer to fire `delta_ticks` timer ticks in the future.
    #[cfg(target_arch = "riscv64")]
    pub fn set_timer(delta_ticks: u64) -> Result<(), ()> {
        unsafe {
            let mut a0 = delta_ticks as isize;
            let a7 = 0x00isize;
            asm!(
                "ecall",
                inout("a0") a0,
                in("a7") a7
            );
            if a0 == 0 { Ok(()) } else { Err(()) }
        }
    }

    pub fn console_putchar(ch: u8) -> Result<(), ()> {
        let mut a0 = ch as isize;
        let a7 = 0x01isize;
        unsafe {
            asm!("ecall", inout("a0") a0, in("a7")a7);
        }
        if a0 == 0 { Ok(()) } else { Err(()) }
    }

    pub fn console_getchar() -> Result<u8, ()> {
        let a0: isize;
        let a7 = 0x02isize;
        unsafe {
            asm!("ecall", out("a0") a0, in("a7")a7);
        }
        if a0 >= 0 { Ok(a0 as u8) } else { Err(()) }
    }
}

/// SBI timer extension.
pub mod timer {
    use super::*;
    const TIME_EID: isize = 0x54494D45;

    pub fn probe() -> bool {
        probe_extension(TIME_EID).is_ok()
    }

    /// Set a timer to fire when the timer hits absolute value `ticks`.
    #[cfg(target_arch = "riscv32")]
    pub fn set_timer(ticks: u64) -> SbiResult {
        sbi_call!(0, TIME_EID, ticks as isize, (ticks >> 32) as isize)
    }

    /// Set a timer to fire when the timer hits absolute value `ticks`.
    #[cfg(target_arch = "riscv64")]
    pub fn set_timer(ticks: u64) -> SbiResult {
        sbi_call!(0, TIME_EID, ticks as isize)
    }
}

/// SBI IPI extension.
pub mod ipi {
    use super::*;
    const IPI_EID: isize = 0x735049;

    pub fn probe() -> bool {
        probe_extension(IPI_EID).is_ok()
    }

    /// Send an IPI to specified HARTs.
    pub fn send_ipi(hart_mask: usize, hart_mask_base: usize) -> SbiResult {
        sbi_call!(0, IPI_EID, hart_mask as isize, hart_mask_base as isize)
    }
}

/// SBI remote fence extension.
pub mod rfence {
    use super::*;
    const RFENCE_EID: isize = 0x52464E43;

    pub fn probe() -> bool {
        probe_extension(RFENCE_EID).is_ok()
    }

    /// Perform a remote `fence.i` on specified HARTs.
    pub fn remote_fence_i(hart_mask: usize, hart_mask_base: usize) -> SbiResult {
        sbi_call!(0, RFENCE_EID, hart_mask as isize, hart_mask_base as isize)
    }

    /// Perform a remote `sfence.vma` without ASID on specified HARTs.
    /// Applies to the entire address space if `start_addr` and `size` are 0, or if `size` is `usize::MAX`.
    pub fn remote_sfence_vma(
        hart_mask: usize,
        hart_mask_base: usize,
        start_addr: *const (),
        size: usize,
    ) -> SbiResult {
        sbi_call!(
            0,
            RFENCE_EID,
            hart_mask as isize,
            hart_mask_base as isize,
            start_addr as isize,
            size as isize
        )
    }

    /// Perform a remote `sfence.vma` with ASID on specified HARTs.
    /// Applies to the entire address space if `start_addr` and `size` are 0, or if `size` is `usize::MAX`.
    pub fn remote_sfence_vma_asid(
        hart_mask: usize,
        hart_mask_base: usize,
        start_addr: *const (),
        size: usize,
        asid: u16,
    ) -> SbiResult {
        sbi_call!(
            0,
            RFENCE_EID,
            hart_mask as isize,
            hart_mask_base as isize,
            start_addr as isize,
            size as isize,
            asid as isize
        )
    }
}

/// SBI hart management extension.
pub mod hsm {
    use crate::cpu::{PhysCpuID, irq};

    use super::*;
    const HSM_EID: isize = 0x48534D;

    pub fn probe() -> bool {
        probe_extension(HSM_EID).is_ok()
    }

    /// HART state.
    #[repr(isize)]
    pub enum HartState {
        Started = 0,
        Stopped = 1,
        StartPending = 2,
        StopPending = 3,
        Suspended = 4,
        SuspendPending = 5,
        ResumePending = 6,
    }

    impl From<isize> for HartState {
        fn from(value: isize) -> Self {
            use HartState::*;
            match value {
                0 => Started,
                1 => Stopped,
                2 => StartPending,
                3 => StopPending,
                4 => Suspended,
                5 => SuspendPending,
                6 => ResumePending,
                _ => panic!("Invalid HART state"),
            }
        }
    }

    /// Start a single HART and set its `a0` register to its HARTID.
    pub unsafe fn start(hartid: PhysCpuID, start_addr: *const (), a1_value: usize) -> SbiResult {
        sbi_call!(
            0,
            HSM_EID,
            hartid as isize,
            start_addr as isize,
            a1_value as isize
        )
    }

    /// Stop this HART; does not return if successful.
    pub fn stop() -> SbiResult {
        debug_assert!(!irq::is_enabled());
        sbi_call!(1, HSM_EID)
    }

    /// Get the status of some HART.
    pub fn get_status(hartid: PhysCpuID) -> SbiResult<HartState> {
        sbi_call!(2, HSM_EID, hartid as isize).map(Into::into)
    }

    /// Suspend this HART.
    /// The suspend type is platform-defined.
    /// If the suspend is non-retentive, the resume sets `a0` to the HARTID.
    pub fn suspend(type_: u32, resume_addr: *const (), a1_value: usize) -> SbiResult {
        sbi_call!(type_ as isize, resume_addr as isize, a1_value as isize)
    }
}
