use crate::bindings::raw::driver_add;

pub mod ns16550a;

pub(super) fn add_drivers() {
    unsafe { driver_add(&raw const ns16550a::NS16550A_DRIVER.base) };
}
