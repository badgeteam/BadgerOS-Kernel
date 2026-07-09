// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    any::Any,
    cmp,
    fmt::{Debug, Display},
    num::NonZeroU32,
    ops::Deref,
    sync::atomic::{self, AtomicU32},
};

use alloc::sync::{Arc, Weak};
use dtb::DtbNode;

use crate::{
    bindings::{
        error::{EResult, Errno},
        raw::timestamp_us_t,
    },
    dev2::Device,
    kernel::sync::{mutex::Mutex, waitlist::Waitlist},
};

use super::registry;

pub mod ata;
pub mod pci;
pub mod soc;

pub(super) struct BusBaseResv {
    pub(super) resv_id: Option<NonZeroU32>,
    pub(super) device: Option<Weak<dyn Device>>,
}

/// Base bus struct; intended for use by implementers of [`Bus`].
pub struct BusBase {
    /// ID assigned by the device registry.
    id: NonZeroU32,
    /// For what device this bus is currently in use, if any.
    pub(super) reservation: Mutex<BusBaseResv>,
    /// How many exclusive calls are in flight.
    /// Some calls (e.g. [`Bus::dtb_node()`]) don't require a reservation and so do not increment this.
    inflight: AtomicU32,
    /// Waiting list for bus reservation cancellation.
    waitlist: Waitlist,
}

impl BusBase {
    /// Create a new device base with a freshly allocated unique ID.
    pub fn new() -> Self {
        Self {
            id: registry::alloc_bus_id(),
            reservation: Mutex::new(BusBaseResv {
                resv_id: None,
                device: None,
            }),
            inflight: AtomicU32::new(0),
            waitlist: Waitlist::new(),
        }
    }
}

impl Drop for BusBase {
    fn drop(&mut self) {
        registry::dealloc_bus_id(self.id);
    }
}

/// A bus is how a driver communicates with devices.
/// Buses may exist on their own (e.g. MMIO) or as part of another device (e.g. AHCI ports).
/// Most of the logic for buses depends on their specific types, this trait serves mostly to register buses.
/// A single bus usually supports multiple devices.
pub trait Bus: Display + Any + Send + Sync + 'static {
    /// Get the base bus struct.
    fn base(&self) -> &BusBase;

    /// Which device provided this bus, if any.
    fn parent_device(&self) -> Option<Arc<dyn Device>>;

    /// Install the handler for an interrupt.
    ///
    /// # Safety
    /// The caller promises to remove the handler with [`Self::uninstall_irq()`] before it becomes invalid.
    ///
    /// The caller promises that the handler is a valid [`Device`] object.
    unsafe fn install_irq(&self, irq_id: u128, device: *const dyn Device) -> EResult<()>;

    /// Remove the handler for an interrupt.
    ///
    /// # Safety
    /// The caller promises that the handler is a valid [`Device`] object.
    unsafe fn uninstall_irq(&self, irq_id: u128, device: *const dyn Device);

    /// Associated DTB node, if any.
    fn dtb_node(&self) -> Option<&'static DtbNode>;

    /// ID assigned by the device registry.
    /// The ID is unique to this bus, even if not discoverable through the registry.
    fn id(&self) -> NonZeroU32 {
        self.base().id
    }
}

impl dyn Bus {
    /// Try to claim this bus for a given device.
    /// If the device becomes unreferenced, the bus is released automatically.
    ///
    /// Using a bus without claiming it will usually work, but multiple drivers contending for one bus
    /// will likely cause unintended behaviour.
    pub fn claim(self: &Arc<Self>) -> EResult<BusResv<dyn Bus>> {
        let base = self.base();
        let mut guard = base.reservation.unintr_lock();
        if guard.resv_id.is_some() {
            return Err(Errno::EADDRINUSE);
        }

        // Wait for in-flight calls from a potential previous reservation to finish.
        while base.inflight.load(atomic::Ordering::Relaxed) != 0 {
            base.waitlist.unintr_block(timestamp_us_t::MAX, || {
                base.inflight.load(atomic::Ordering::Relaxed) != 0
            });
        }

        // Allocate new ID for this reservation.
        let resv_id = registry::alloc_resv_id();
        guard.resv_id = Some(resv_id);
        Ok(BusResv(BusResvInner {
            bus: self.clone(),
            dyn_bus: self.clone(),
            resv_id,
        }))
    }

    /// Common implementation of [`Self::cancel_resv()`] and [`BusResv::cancel_resv()`].
    fn cancel_resv_from(&self, id: Option<NonZeroU32>) {
        let base = self.base();
        let mut guard = base.reservation.unintr_lock();
        if guard.resv_id.is_none() {
            return;
        }
        if let Some(id) = id
            && let Some(id2) = guard.resv_id
            && id != id2
        {
            return;
        }

        guard.device = None;
        guard.resv_id = None;

        drop(guard);
    }

    /// Cancel the reservation on this bus.
    /// After this, remaining exclusive calls on the bus will fail with [`Errno::ENODEV`].
    pub fn cancel_resv(&self) {
        self.cancel_resv_from(None);
    }

    /// Check which device currently owns this bus.
    pub fn owner(&self) -> Option<Arc<dyn Device>> {
        self.base()
            .reservation
            .unintr_lock_shared()
            .device
            .as_ref()?
            .upgrade()
    }
}

impl PartialEq for dyn Bus {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}
impl Eq for dyn Bus {}

impl PartialOrd for dyn Bus {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for dyn Bus {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.id().cmp(&other.id())
    }
}

/// In-flight operation on a [`BusResv`].
pub struct BusInflight<'a, T: ?Sized + Bus>(&'a T);

impl<'a, T: ?Sized + Bus> Deref for BusInflight<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, T: ?Sized + Bus> Drop for BusInflight<'a, T> {
    fn drop(&mut self) {
        let base = self.0.base();
        if base.inflight.fetch_sub(1, atomic::Ordering::Relaxed) == 1 {
            base.waitlist.notify_all();
        }
    }
}

/// Reservation on a [`Bus`]; allows buses to be dynamically detached from devices.
#[repr(transparent)]
pub struct BusResv<T: ?Sized + Bus>(BusResvInner<T>);

struct BusResvInner<T: ?Sized + Bus> {
    bus: Arc<T>,
    dyn_bus: Arc<dyn Bus>,
    resv_id: NonZeroU32,
}

impl<T: ?Sized + Bus> Debug for BusResv<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0.dyn_bus, f)
    }
}

impl<T: ?Sized + Bus> Drop for BusResv<T> {
    fn drop(&mut self) {
        self.cancel_resv();
        registry::dealloc_resv_id(self.0.resv_id);
    }
}

impl<T: Bus> BusResv<T> {
    /// Convert a specialized bus reservation back into a generic one.
    pub fn into_dyn(self) -> BusResv<dyn Bus> {
        let this: BusResvInner<T> = unsafe { core::mem::transmute(self) };
        BusResv(BusResvInner {
            bus: this.bus,
            dyn_bus: this.dyn_bus,
            resv_id: this.resv_id,
        })
    }
}

impl<T: ?Sized + Bus> BusResv<T> {
    /// Cancel the reservation on this bus.
    /// After this, remaining exclusive calls on the bus will fail with [`Errno::ENODEV`].
    pub fn cancel_resv(&self) {
        self.0.dyn_bus.cancel_resv_from(Some(self.0.resv_id));
    }

    /// Get the bus without checking whether it is still claimed.
    ///
    /// # Safety
    /// The caller promises not to run operations that could interfere with other drivers.
    pub unsafe fn take_unchecked(&self) -> &T {
        &self.0.bus
    }

    /// Run an operation on the bus if it is still claimed.
    #[inline]
    pub fn take<'a>(&'a self) -> EResult<BusInflight<'a, T>> {
        let base = self.0.dyn_bus.base();
        let guard = base.reservation.unintr_lock_shared();
        if guard.resv_id != Some(self.0.resv_id) {
            return Err(Errno::ENODEV);
        }
        base.inflight.fetch_add(1, atomic::Ordering::Relaxed);
        Ok(BusInflight(&self.0.bus))
    }

    /// Install the handler for an interrupt.
    ///
    /// # Safety
    /// The caller promises to remove the handler with [`Self::uninstall_irq()`] before it becomes invalid.
    ///
    /// The caller promises that the handler is a valid [`Device`] object.
    pub unsafe fn install_irq(&self, irq_id: u128, device: *const dyn Device) -> EResult<()> {
        unsafe { self.take()?.install_irq(irq_id, device) }
    }

    /// Remove the handler for an interrupt.
    ///
    /// # Safety
    /// The caller promises that the handler is a valid [`Device`] object.
    pub unsafe fn uninstall_irq(&self, irq_id: u128, device: *const dyn Device) {
        // We allow uninstalling IRQs even if the device no longer holds the reservation.
        unsafe { self.0.bus.uninstall_irq(irq_id, device) };
    }

    /// Associated DTB node, if any.
    pub fn dtb_node(&self) -> Option<&'static DtbNode> {
        self.0.bus.dtb_node()
    }

    /// ID assigned by the device registry.
    /// The ID is unique to this bus, even if not discoverable through the registry.
    pub fn id(&self) -> NonZeroU32 {
        self.0.bus.id()
    }
}

impl BusResv<dyn Bus> {
    /// Try to downcast a generic bus reservation into a specialized one.
    pub fn downcast<T: Bus>(self) -> Result<BusResv<T>, Self> {
        let this: BusResvInner<dyn Bus> = unsafe { core::mem::transmute(self) };
        if !(&*this.bus as &dyn Any).is::<T>() {
            return Err(Self(this));
        } else {
            return Ok(BusResv(BusResvInner {
                bus: unsafe { Arc::downcast_unchecked(this.bus) },
                dyn_bus: this.dyn_bus,
                resv_id: this.resv_id,
            }));
        }
    }
}

impl<T: Bus> Display for BusResv<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.bus.fmt(f)
    }
}
