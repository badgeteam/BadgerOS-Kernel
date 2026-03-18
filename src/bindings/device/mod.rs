#[cfg(feature = "dtb")]
use crate::bindings::device::dtb::DtbNode;
use crate::{
    LogLevel,
    bindings::{
        self,
        raw::{dev_class_t, dev_filter_t, set_ent_t},
    },
    filesystem::File,
    kernel::sync::mutex::{RawMutex, SharedMutexGuard},
    logkf,
};
use core::{num::NonZero, ptr::NonNull};

use addr::DevAddr;
use alloc::vec::Vec;
use class::{block::BlockDevice, char::CharDevice, pcictl::PciCtlDevice, tty::TTYDevice};

pub mod addr;
pub mod class;
#[cfg(feature = "dtb")]
pub mod dtb;

use crate::bindings::raw::mutex_t;

use super::{
    error::{EResult, Errno},
    raw::{
        self, dev_addr_t, dev_class_t_DEV_CLASS_BLOCK, dev_class_t_DEV_CLASS_CHAR,
        dev_class_t_DEV_CLASS_PCICTL, dev_class_t_DEV_CLASS_TTY, dev_class_t_DEV_CLASS_UNKNOWN,
        dev_state_t, device_block_t, device_char_t, device_info_t, device_pcictl_t, device_t,
        device_tty_t, driver_t, dtb_handle_t, dtb_node_t, irqno_t, set_next,
    },
};

/// Device info.
#[derive(Clone)]
pub struct DeviceInfo {
    pub parent: Option<Device>,
    pub irq_parent: Option<Device>,
    pub addrs: Vec<DevAddr>,
    // TODO: DTB handle.
    pub phandle: Option<NonZero<u32>>,
}

impl DeviceInfo {
    /// Converts into the format required by the C API.
    pub fn into_raw(self) -> device_info_t {
        if let Some(ref parent) = self.parent {
            unsafe { raw::device_push_ref(parent.base_ptr()) };
        }
        if let Some(ref irq_parent) = self.irq_parent {
            unsafe { raw::device_push_ref(irq_parent.base_ptr()) };
        }
        let addrs: Vec<dev_addr_t> = self.addrs.into_iter().map(Into::into).collect();
        let addrs = addrs.into_parts();
        device_info_t {
            parent: self
                .parent
                .map(|x| x.leak().as_ptr())
                .unwrap_or(0 as *mut device_t),
            irq_parent: self
                .irq_parent
                .map(|x| x.leak().as_ptr())
                .unwrap_or(0 as *mut device_t),
            addrs_len: addrs.1,
            addrs: addrs.0.as_ptr(),
            dtb_handle: 0 as *mut dtb_handle_t,
            dtb_node: 0 as *mut dtb_node_t,
            phandle: self.phandle.map(Into::into).unwrap_or(0u32),
        }
    }
}

/// View of an existing device's info.
#[derive(Clone, Copy)]
pub struct DeviceInfoView<'a> {
    inner: &'a device_info_t,
}

impl<'a> DeviceInfoView<'a> {
    pub unsafe fn from(inner: &'a device_info_t) -> Self {
        Self { inner }
    }
    pub fn parent(&self) -> Option<Device> {
        if self.inner.parent.is_null() {
            return None;
        }
        unsafe { raw::device_push_ref(self.inner.parent) }
        Some(unsafe { Device::from_raw(self.inner.parent) })
    }
    pub fn irq_parent(&self) -> Option<Device> {
        if self.inner.irq_parent.is_null() {
            return None;
        }
        unsafe { raw::device_push_ref(self.inner.irq_parent) }
        Some(unsafe { Device::from_raw(self.inner.irq_parent) })
    }
    pub fn addrs(&'a self) -> &'a [dev_addr_t] {
        unsafe { &*core::ptr::slice_from_raw_parts(self.inner.addrs, self.inner.addrs_len) }
    }
    #[cfg(feature = "dtb")]
    pub fn dtb(&self) -> Option<&'static DtbNode> {
        unsafe { core::mem::transmute(self.inner.dtb_node) }
    }
    pub fn phandle(&self) -> Option<NonZero<u32>> {
        NonZero::try_from(self.inner.phandle).ok()
    }
    #[cfg(feature = "dtb")]
    pub fn dtb_match(&self, supported: &[&str]) -> bool {
        let compatible: Option<_> = try { self.dtb()?.get_prop("compatible")?.bytes() };
        let compatible = match compatible {
            Some(x) => x,
            None => return false,
        };

        for substr in compatible.split(|x| *x == 0) {
            for substr2 in supported {
                if *substr == *substr2.as_bytes() {
                    return true;
                }
            }
        }

        false
    }
}

/// Abstract base class for all devices.
pub struct AbstractDevice<T: Sized> {
    inner: NonNull<T>,
}
unsafe impl<T> Send for AbstractDevice<T> {}
unsafe impl<T> Sync for AbstractDevice<T> {}

impl<T> Drop for AbstractDevice<T> {
    fn drop(&mut self) {
        unsafe { raw::device_pop_ref(self.inner.as_ptr() as *mut device_t) }
    }
}

impl<T> Clone for AbstractDevice<T> {
    fn clone(&self) -> Self {
        unsafe { raw::device_push_ref(self.inner.as_ptr() as *mut device_t) };
        Self { inner: self.inner }
    }
}

impl<T> DeviceFromRaw<T, AbstractDevice<T>> for AbstractDevice<T> {
    /// Create from a raw pointer (doesn't check type).
    /// Increments the refcount.
    unsafe fn from_raw_ref(inner: *mut T) -> Self {
        unsafe {
            raw::device_push_ref(inner as *mut device_t);
            Self {
                inner: NonNull::from_mut(&mut *inner),
            }
        }
    }
    /// Create from a raw pointer (doesn't check type).
    /// Doesn't increment the refcount.
    unsafe fn from_raw(inner: *mut T) -> Self {
        unsafe {
            Self {
                inner: NonNull::from_mut(&mut *inner),
            }
        }
    }
}

impl<T> AbstractDevice<T> {
    /// Leak the device, returning the raw pointer.
    /// This does not decrement the refcount.
    pub fn leak(self) -> NonNull<T> {
        let ptr = self.inner;
        core::mem::forget(self);
        ptr
    }
    /// Get the raw pointer.
    pub unsafe fn as_raw_ptr(&self) -> *mut T {
        self.inner.as_ptr()
    }
    /// Try to cast into specialized device.
    pub fn specialized(self) -> Device {
        let res = unsafe { Device::from_raw(core::mem::transmute(self.inner)) };
        core::mem::forget(self);
        res
    }
}

impl<T> HasBaseDevice for AbstractDevice<T> {
    /// Borrow as a handle to the base device type.
    fn as_base<'a>(&self) -> &'a BaseDevice {
        unsafe { core::mem::transmute(self) }
    }
    /// Get the raw pointer to the base struct.
    fn base_ptr(&self) -> *mut device_t {
        self.inner.as_ptr() as *mut device_t
    }
}

pub type BaseDevice = AbstractDevice<device_t>;
impl BaseDevice {
    /// Get a list of devices using a filter.
    pub fn filter(filters: DeviceFilters) -> EResult<Vec<BaseDevice>> {
        unsafe {
            Device::filter_impl::<device_t, BaseDevice, true>(
                filters,
                dev_class_t_DEV_CLASS_UNKNOWN,
            )
        }
    }
}

/// Helper trait to implement base device functions.
pub trait HasBaseDevice {
    /// Borrow as a handle to the base device type.
    fn as_base<'a>(&self) -> &'a BaseDevice;
    /// Get the raw pointer to the base struct.
    fn base_ptr(&self) -> *mut device_t;
    /// Get the device class.
    fn class(&self) -> dev_class_t {
        unsafe { *self.base_ptr() }.dev_class
    }

    /// Get the parent device, if any.
    fn parent(&self) -> Option<Device> {
        self.info().parent()
    }
    /// Get the device's children.
    fn children(&self) -> EResult<Vec<Device>> {
        Device::filter(DeviceFilters {
            parent: Some(self.id().into()),
            ..Default::default()
        })
    }
    /// Get device info view.
    fn info<'a>(&'a self) -> DeviceInfoView<'a> {
        unsafe {
            DeviceInfoView {
                inner: &(*self.base_ptr()).info,
            }
        }
    }
    /// Get device ID.
    fn id(&self) -> NonZero<u32> {
        unsafe { core::mem::transmute((*self.base_ptr()).id) }
    }
    /// Get device state view.
    fn state<'a>(&'a self) -> dev_state_t {
        unsafe extern "C" {
            static mut devs_mtx: mutex_t;
        }
        unsafe {
            raw::mutex_lock_shared(&raw mut devs_mtx);
            let res = (*self.base_ptr()).state;
            raw::mutex_unlock_shared(&raw mut devs_mtx);
            res
        }
    }
    /// Get device driver view.
    fn driver<'a>(&'a self) -> *const driver_t {
        unsafe extern "C" {
            static mut devs_mtx: mutex_t;
        }
        unsafe {
            raw::mutex_lock_shared(&raw mut (*self.base_ptr()).driver_mtx);
            let res = (*self.base_ptr()).driver;
            raw::mutex_unlock_shared(&raw mut (*self.base_ptr()).driver_mtx);
            res
        }
    }
    /// Try to activate this device.
    /// Does nothing if the device is not in-tree or already activated.
    fn activate(&self) {
        unsafe { raw::device_activate(self.base_ptr()) }
    }
    /// Try to get this as a block device.
    fn as_block(&self) -> Option<BlockDevice> {
        unsafe {
            if (*self.base_ptr()).dev_class != dev_class_t_DEV_CLASS_BLOCK {
                None
            } else {
                Some(BlockDevice::from_raw_ref(
                    self.base_ptr() as *mut device_block_t
                ))
            }
        }
    }
    /// Try to get this as a PCI controller.
    fn as_pcictl(&self) -> Option<PciCtlDevice> {
        unsafe {
            if (*self.base_ptr()).dev_class != dev_class_t_DEV_CLASS_PCICTL {
                None
            } else {
                Some(PciCtlDevice::from_raw_ref(
                    self.base_ptr() as *mut device_pcictl_t
                ))
            }
        }
    }
    /// Try to get this as a character device.
    fn as_char(&self) -> Option<CharDevice> {
        unsafe {
            if (*self.base_ptr()).dev_class != dev_class_t_DEV_CLASS_CHAR
                && (*self.base_ptr()).dev_class != dev_class_t_DEV_CLASS_TTY
            {
                None
            } else {
                Some(CharDevice::from_raw_ref(
                    self.base_ptr() as *mut device_char_t
                ))
            }
        }
    }
    /// Try to get this as a TTY device.
    fn as_tty(&self) -> Option<TTYDevice> {
        unsafe {
            if (*self.base_ptr()).dev_class != dev_class_t_DEV_CLASS_TTY {
                None
            } else {
                Some(TTYDevice::from_raw_ref(self.base_ptr() as *mut device_tty_t))
            }
        }
    }
    /// Cascade-enable an outgoing interrupt.
    unsafe fn cascase_enable_irq_out(&self, irqno: irqno_t) -> EResult<()> {
        unsafe { Errno::check(raw::device_cascade_enable_irq_out(self.base_ptr(), irqno)) }
    }
    /// Helper to send an interrupt to all children on a certain designator.
    /// Returns true if an interrupt handler was run.
    unsafe fn forward_interrupt(&self, in_irqno: irqno_t) -> bool {
        unsafe { raw::device_forward_interrupt(self.base_ptr(), in_irqno) }
    }
    /// Send an interrupt to this device.
    unsafe fn interrupt(&self, irqno: irqno_t) -> bool {
        unsafe { raw::device_interrupt(self.base_ptr(), irqno) }
    }
}

/// Represents a bit-masked filter for device addresses.
#[derive(Clone, Copy, Debug)]
pub struct DevAddrFilter {
    pub addr: DevAddr,
    pub mask: Option<DevAddr>,
}

/// A set of filters that can be used to look for devices.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeviceFilters {
    /// Match devices with similar addresses.
    pub addr: Option<DevAddrFilter>,
    /// Match devices with this as their parent.
    pub parent: Option<u32>,
    /// Match devices with this as their driver.
    pub driver: Option<NonNull<driver_t>>,
}

impl Into<dev_filter_t> for DeviceFilters {
    fn into(self) -> dev_filter_t {
        let mut filters: dev_filter_t = unsafe { core::mem::zeroed() };
        if let Some(addr) = self.addr {
            filters.match_addr = true;
            filters.addr = addr.addr.into();
            if let Some(mask) = addr.mask {
                filters.use_addr_mask = true;
                filters.addr_mask = mask.into();
            }
        }
        if let Some(parent) = self.parent {
            filters.match_parent = true;
            filters.parent_id = parent;
        }
        if let Some(driver) = self.driver {
            filters.driver = driver.as_ptr();
        }
        filters
    }
}

pub trait DeviceFromRaw<S: Sized, T: Sized> {
    /// Create a device  from a raw pointer.
    /// Doesn't increment the refcount.
    unsafe fn from_raw(ptr: *mut S) -> T;
    /// Create a device enum from a raw pointer.
    /// Increments the refcount.
    unsafe fn from_raw_ref(inner: *mut S) -> T;
}

/// Enum that encapsulates all types of device.
#[derive(Clone)]
#[repr(u32, C)]
pub enum Device {
    Unknown(BaseDevice) = dev_class_t_DEV_CLASS_UNKNOWN,
    Block(BlockDevice) = dev_class_t_DEV_CLASS_BLOCK,
    TTY(TTYDevice) = dev_class_t_DEV_CLASS_TTY,
    Char(CharDevice) = dev_class_t_DEV_CLASS_CHAR,
    PciCtl(PciCtlDevice) = dev_class_t_DEV_CLASS_PCICTL,
}

impl DeviceFromRaw<device_t, Device> for Device {
    /// Create a device enum from a raw pointer.
    /// Doesn't increment the refcount.
    unsafe fn from_raw(inner: *mut device_t) -> Self {
        unsafe {
            #[allow(non_upper_case_globals)]
            match (*inner).dev_class {
                dev_class_t_DEV_CLASS_UNKNOWN => Device::Unknown(BaseDevice::from_raw(inner)),
                dev_class_t_DEV_CLASS_BLOCK => {
                    Device::Block(BlockDevice::from_raw(inner as *mut device_block_t))
                }
                dev_class_t_DEV_CLASS_PCICTL => {
                    Device::PciCtl(PciCtlDevice::from_raw(inner as *mut device_pcictl_t))
                }
                dev_class_t_DEV_CLASS_TTY => {
                    Device::TTY(TTYDevice::from_raw(inner as *mut device_tty_t))
                }
                dev_class_t_DEV_CLASS_CHAR => {
                    Device::Char(CharDevice::from_raw(inner as *mut device_char_t))
                }
                _ => {
                    logkf!(
                        LogLevel::Warning,
                        "Unknown device class {}",
                        (*inner).dev_class
                    );
                    Device::Unknown(BaseDevice::from_raw(inner))
                }
            }
        }
    }
    /// Create a device enum from a raw pointer.
    /// Increments the refcount.
    unsafe fn from_raw_ref(inner: *mut device_t) -> Self {
        unsafe {
            raw::device_push_ref(inner);
            Self::from_raw(inner)
        }
    }
}

impl Device {
    /// Get a list of devices using a filter.
    unsafe fn filter_impl<S: Sized, F: DeviceFromRaw<S, F>, const CHECK: bool>(
        filters: DeviceFilters,
        class: dev_class_t,
    ) -> EResult<Vec<F>> {
        unsafe {
            let mut filters: dev_filter_t = filters.into();
            filters.match_class = CHECK;
            filters.class = class;
            let mut devs = raw::device_get_filtered(&raw const filters);
            let mut vec = Vec::new();
            let mut iter = raw::set_next(&raw const devs, 0 as *const set_ent_t);
            let mut oom = false;
            while !iter.is_null() {
                let dev = F::from_raw((*iter).value as *mut S);
                if !oom && vec.try_reserve(vec.len() + 1).is_err() {
                    oom = true;
                }
                if !oom {
                    vec.push(dev);
                }
                iter = raw::set_next(&raw const devs, iter);
            }
            raw::set_clear(&raw mut devs);
            if oom { Err(Errno::ENOMEM) } else { Ok(vec) }
        }
    }
    /// Get a list of devices using a filter.
    pub fn filter(filters: DeviceFilters) -> EResult<Vec<Device>> {
        unsafe {
            Self::filter_impl::<device_t, Device, false>(filters, dev_class_t_DEV_CLASS_UNKNOWN)
        }
    }
    /// Try to get a device by ID.
    pub fn by_id(id: u32) -> Option<BaseDevice> {
        unsafe {
            let res = raw::device_by_id(id);
            if res.is_null() {
                None
            } else {
                Some(BaseDevice::from_raw(res))
            }
        }
    }
    /// Leak the device, returning the raw pointer.
    /// This does not decrement the refcount.
    pub fn leak(self) -> NonNull<device_t> {
        unsafe {
            match self {
                Device::Unknown(x) => x.leak(),
                Device::Block(x) => core::mem::transmute(x.leak()),
                Device::PciCtl(x) => core::mem::transmute(x.leak()),
                Device::TTY(x) => core::mem::transmute(x.leak()),
                Device::Char(x) => core::mem::transmute(x.leak()),
            }
        }
    }
    /// Add a new device.
    pub fn add(info: DeviceInfo) -> EResult<BaseDevice> {
        let raw = unsafe { raw::device_add(info.into_raw()) };
        if raw.is_null() {
            Err(Errno::ENOMEM)
        } else {
            Ok(unsafe { BaseDevice::from_raw(raw) })
        }
    }
    /// Remove a device by ID.
    pub fn remove(id: NonZero<u32>) -> bool {
        unsafe { raw::device_remove(id.into()) }
    }
    /// Add a device interrupt link; child is the device that generates the interrupt, parent the one that receives it.
    /// Any device interrupt designator can be connected to any number of opposite designators, but the resulting graph must be acyclic.
    pub unsafe fn link_irq(
        child: &dyn HasBaseDevice,
        child_irqno: irqno_t,
        parent: &dyn HasBaseDevice,
        parent_irqno: irqno_t,
    ) -> EResult<()> {
        Errno::check(unsafe {
            raw::device_link_irq(
                child.base_ptr(),
                child_irqno,
                parent.base_ptr(),
                parent_irqno,
            )
        })
    }
    /// Remove a device interrupt link; see [`Device::link_irq`].
    pub unsafe fn unlink_irq(
        child: &dyn HasBaseDevice,
        child_irqno: irqno_t,
        parent: &dyn HasBaseDevice,
        parent_irqno: irqno_t,
    ) -> EResult<()> {
        Errno::check(unsafe {
            raw::device_unlink_irq(
                child.base_ptr(),
                child_irqno,
                parent.base_ptr(),
                parent_irqno,
            )
        })
    }
}

impl HasBaseDevice for Device {
    /// Borrow as a handle to the base device type.
    fn as_base<'a>(&self) -> &'a BaseDevice {
        match self {
            Device::Unknown(x) => x.as_base(),
            Device::Block(x) => x.as_base(),
            Device::PciCtl(x) => x.as_base(),
            Device::TTY(x) => x.as_base(),
            Device::Char(x) => x.as_base(),
        }
    }

    /// Get the raw pointer to the base struct.
    fn base_ptr(&self) -> *mut device_t {
        match self {
            Device::Unknown(x) => x.base_ptr(),
            Device::Block(x) => x.base_ptr(),
            Device::PciCtl(x) => x.base_ptr(),
            Device::TTY(x) => x.base_ptr(),
            Device::Char(x) => x.base_ptr(),
        }
    }
}

/// Common device driver functions.
pub trait BaseDriver: Sync {
    /// Post-add callback used for e.g. enabling interrupts.
    fn post_add(&self) {}
    /// Remove a device from this driver; only called once.
    fn remove(&mut self) {}
    /// [optional] Called after a direct child device is added with [`Device::add`].
    /// If this fails, the child is removed again.
    fn child_added(&self, _child: BaseDevice) -> EResult<()> {
        Ok(())
    }
    /// [optional] Called after a direct child is activated with [`HasBaseDevice::activate`].
    fn child_activated(&self, _child: BaseDevice) {}
    /// [optional] Called after a direct child device gets added to a driver.
    fn child_got_driver(&self, _child: BaseDevice) {}
    /// [optional] Called before a direct child device gets removed from a driver.
    /// Always called before `child_removed`.
    fn child_lost_driver(&self, _child: BaseDevice) {}
    /// [optional] Called before a direct child device is removed with [`Device::remove`].
    fn child_removed(&self, _child: BaseDevice) {}
    /// Device interrupt handler; also responsible for any potential forwarding of interrupts.
    /// Only called from an interrupt context.
    /// Returns true if this handled an interrupt request.
    fn interrupt(&self, _irq: irqno_t) -> bool {
        false
    }
    /// Enable a certain interrupt output.
    /// Can be called with interrupts disabled.
    fn enable_irq_out(&self, _irq: irqno_t, _enable: bool) -> EResult<()> {
        Err(Errno::ENOTSUP)
    }
    /// [optional] Enable an incoming interrupt.
    /// Can be called with interrupts disabled.
    fn enable_irq_in(&self, _irq: irqno_t, _enable: bool) -> EResult<()> {
        Err(Errno::ENOTSUP)
    }
    /// [optional] Cascade-enable interrupts from some input designator.
    /// Can be called with interrupts disabled.
    fn cascase_enable_irq(&self, _irq: irqno_t) -> EResult<()> {
        Err(Errno::ENOTSUP)
    }
    /// [optional] Create additional device node files.
    /// Called when a new `devtmpfs` is mounted OR after registered to the driver.
    fn create_devnodes(&self, _devtmpfs_root: &dyn File, _devnode_dir: &dyn File) -> EResult<()> {
        Ok(())
    }
}

/// Helper macro for filling in driver fields.
#[macro_export]
macro_rules! abstract_driver_struct {
    ($type: ty, $class: expr, $match_: expr, $add: expr) => {{
        use crate::{filesystem::c_api::*, bindings::{error::*, device::*, raw::*}};
        use ::alloc::boxed::Box;
        use ::core::ffi::c_void;
        driver_t {
            dev_class: $class,
            match_: {
                /// Convert the types and call the matching function.
                extern "C" fn match_wrapper(info: *mut device_info_t) -> bool {
                    $match_(unsafe { DeviceInfoView::from(&*info) })
                }
                Some(match_wrapper)
            },
            add: {
                /// Convert the types and call the driver's add function and unbox the object if successful.
                extern "C" fn add_wrapper(
                    device: *mut device_t,
                ) -> errno_t {
                    let res: EResult<Box<$type>> = $add(unsafe { Device::from_raw_ref(device) });
                    match res {
                        Ok(b) => {
                            let ptr = Box::into_raw(b);
                            unsafe {
                                (*device).cookie = ptr as *mut c_void;
                            }
                            0
                        }
                        Err(errno) => -(errno as u32 as i32),
                    }
                }
                Some(add_wrapper)
            },
            post_add: {
                unsafe extern "C" fn post_add_wrapper(device: *mut device_t) {
                    let ptr = unsafe{&mut *((*device).cookie as *mut $type)};
                    ptr.post_add()
                }
                Some(post_add_wrapper)
            },
            remove: {
                unsafe extern "C" fn remove_wrapper(device: *mut device_t) {
                    let ptr = unsafe{&mut *((*device).cookie as *mut $type)};
                    ptr.remove()
                }
                Some(remove_wrapper)
            },
            child_added: {
                unsafe extern "C" fn child_added_wrapper(device: *mut device_t, child: *mut device_t) -> errno_t {
                    let ptr = unsafe{&mut *((*device).cookie as *mut $type)};
                    let child = unsafe {BaseDevice::from_raw_ref(child)};
                    Errno::extract(ptr.child_added(child))
                }
                Some(child_added_wrapper)
            },
            child_activated: {
                unsafe extern "C" fn child_activated_wrapper(device: *mut device_t, child: *mut device_t) {
                    let ptr = unsafe{&mut *((*device).cookie as *mut $type)};
                    let child = unsafe {BaseDevice::from_raw_ref(child)};
                    ptr.child_activated(child)
                }
                Some(child_activated_wrapper)
            },
            child_got_driver: {
                unsafe extern "C" fn child_got_driver_wrapper(device: *mut device_t, child: *mut device_t) {
                    let ptr = unsafe{&mut *((*device).cookie as *mut $type)};
                    let child = unsafe {BaseDevice::from_raw_ref(child)};
                    ptr.child_got_driver(child)
                }
                Some(child_got_driver_wrapper)
            },
            child_lost_driver: {
                unsafe extern "C" fn child_lost_driver_wrapper(device: *mut device_t, child: *mut device_t) {
                    let ptr = unsafe{&mut *((*device).cookie as *mut $type)};
                    let child = unsafe {BaseDevice::from_raw_ref(child)};
                    ptr.child_lost_driver(child)
                }
                Some(child_lost_driver_wrapper)
            },
            child_removed: {
                unsafe extern "C" fn child_removed_wrapper(device: *mut device_t, child: *mut device_t) {
                    let ptr = unsafe{&mut *((*device).cookie as *mut $type)};
                    let child = unsafe {BaseDevice::from_raw_ref(child)};
                    ptr.child_removed(child)
                }
                Some(child_removed_wrapper)
            },
            interrupt: {
                unsafe extern "C" fn interrupt_wrapper(device: *mut device_t, irqno: irqno_t) -> bool {
                    let ptr = unsafe{&mut *((*device).cookie as *mut $type)};
                    ptr.interrupt(irqno)
                }
                Some(interrupt_wrapper)
            },
            enable_irq_out: {
                unsafe extern "C" fn enable_irq_out_wrapper(device: *mut device_t, irqno: irqno_t, enable: bool) -> errno_t {
                    let ptr = unsafe{&mut *((*device).cookie as *mut $type)};
                    Errno::extract(ptr.enable_irq_out(irqno, enable))
                }
                Some(enable_irq_out_wrapper)
            },
            enable_irq_in: {
                unsafe extern "C" fn enable_irq_in_wrapper(device: *mut device_t, irqno: irqno_t, enable: bool) -> errno_t {
                    let ptr = unsafe{&mut *((*device).cookie as *mut $type)};
                    Errno::extract(ptr.enable_irq_in(irqno, enable))
                }
                Some(enable_irq_in_wrapper)
            },
            cascase_enable_irq: {
                unsafe extern "C" fn cascase_enable_irq_wrapper(device: *mut device_t, irqno: irqno_t) -> errno_t {
                    let ptr = unsafe{&mut *((*device).cookie as *mut $type)};
                    Errno::extract(ptr.cascase_enable_irq(irqno))
                }
                Some(cascase_enable_irq_wrapper)
            },
            create_devnodes: {
                unsafe extern "C" fn create_devnodes_wrapper(device: *mut device_t, devtmpfs_root: file_t, devnode_dir: file_t) -> errno_t {
                    let ptr = unsafe{&mut *((*device).cookie as *mut $type)};
                    let devtmpfs_root = unsafe { file_as_ref(devtmpfs_root) }.unwrap();
                    let devnode_dir = unsafe { file_as_ref(devnode_dir) }.unwrap();
                    Errno::extract(ptr.create_devnodes(devtmpfs_root, devnode_dir))
                }
                Some(create_devnodes_wrapper)
            },
        }
    }};
}

/// Helper macro for filling in base driver fields.
#[macro_export]
macro_rules! base_driver_struct {
    ($type: ty, $match_: expr, $add: expr) => {
        crate::abstract_driver_struct!(
            $type,
            crate::bindings::raw::dev_class_t_DEV_CLASS_UNKNOWN,
            $match_,
            $add
        )
    };
}

pub fn add_driver(driver: &'static driver_t) -> EResult<()> {
    Errno::check(unsafe { raw::driver_add(driver) })
}

pub fn remove_driver(driver: &'static driver_t) -> EResult<()> {
    Errno::check(unsafe { raw::driver_remove(driver) })
}

pub fn iter_drivers(mut cb: impl FnMut(SharedMutexGuard<'static, driver_t>) -> bool) {
    unsafe {
        bindings::raw::mutex_lock_shared(&raw mut bindings::raw::drivers_mtx);

        let mut iter = set_next(&raw const bindings::raw::drivers, 0 as *const set_ent_t);
        while !iter.is_null() {
            let guard = SharedMutexGuard::from_raw(
                (&*(&raw const bindings::raw::drivers_mtx as *const RawMutex)).unintr_lock_shared(),
                &*((*iter).value as *const driver_t),
            );
            if !cb(guard) {
                break;
            }
            iter = set_next(&raw const bindings::raw::drivers, iter);
        }

        bindings::raw::mutex_unlock(&raw mut bindings::raw::drivers_mtx);
    };
}
