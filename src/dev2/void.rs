// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::fmt::Display;

use alloc::{sync::Arc, vec::Vec};

use crate::{
    bindings::error::EResult,
    dev2::{Device, DeviceBase, class::char::CharDevice, registry},
    device_get_trait_vtable,
    filesystem::poll,
    kernel::sync::waitlist::Waitlist,
    process::usercopy::{UserSlice, UserSliceMut},
};

struct CharVoid {
    /// Base device structs.
    base: DeviceBase,
    /// If true, fill read buffer with zeroes; if false, reads return no data.
    pub reads_zeroes: bool,
}

impl Display for CharVoid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.reads_zeroes {
            f.write_str("CharVoid(zeroes)")
        } else {
            f.write_str("CharVoid(null)")
        }
    }
}

impl Device for CharVoid {
    fn base(&self) -> &super::DeviceBase {
        &self.base
    }

    fn interrupt(&self, _id: u128) -> bool {
        unreachable!()
    }

    device_get_trait_vtable!(CharDevice);
}

impl CharDevice for CharVoid {
    fn is_tty(&self) -> bool {
        false
    }

    fn poll(&self) -> u32 {
        if self.reads_zeroes {
            poll::IN | poll::OUT
        } else {
            poll::OUT
        }
    }

    fn poll_waitlists<'a>(
        &'a self,
        _interest: u32,
        _collect: &mut Vec<&'a Waitlist>,
    ) -> EResult<()> {
        Ok(())
    }

    fn read_raw(&self, mut rdata: UserSliceMut<u8>, _nonblock: bool) -> EResult<usize> {
        if self.reads_zeroes {
            rdata.fill(0)?;
            return Ok(rdata.len());
        } else {
            return Ok(0);
        }
    }

    fn write_raw(&self, wdata: UserSlice<u8>, _nonblock: bool) -> EResult<usize> {
        Ok(wdata.len())
    }
}

static mut NULL_INSTANCE: Option<Arc<CharVoid>> = None;
static mut ZERO_INSTANCE: Option<Arc<CharVoid>> = None;

/// Get the instance of `/dev/null`.
pub fn null_instance() -> Arc<dyn CharDevice> {
    unsafe { &*&raw const NULL_INSTANCE }.clone().unwrap()
}

/// Get the instance of `/dev/zero`.
pub fn zero_instance() -> Arc<dyn CharDevice> {
    unsafe { &*&raw const ZERO_INSTANCE }.clone().unwrap()
}

/// Create `/dev/null` and `/dev/zero`.
pub(super) unsafe fn init() {
    let null = Arc::new(CharVoid {
        base: DeviceBase::with_node_name("null".into(), true),
        reads_zeroes: false,
    });
    let zero = Arc::new(CharVoid {
        base: DeviceBase::with_node_name("zero".into(), true),
        reads_zeroes: true,
    });

    registry::register_device(null.clone()).expect("Failed to register /dev/null");
    registry::register_device(zero.clone()).expect("Failed to register /dev/zero");

    unsafe {
        NULL_INSTANCE = Some(null);
        ZERO_INSTANCE = Some(zero);
    }
}
