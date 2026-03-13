use core::mem::MaybeUninit;

use alloc::vec::Vec;

use crate::{
    bindings::{
        device::{AbstractDevice, Device, DeviceFilters, addr::PciAddr},
        error::{EResult, Errno},
        raw::{self, dev_class_t_DEV_CLASS_PCICTL, device_pcictl_t, pci_bar_info_t},
    },
    config,
    mem::vmm,
};

/// Representation of some mapped PCIe BAR memory.
pub struct MappedBar {
    info: pci_bar_info_t,
    vaddr: usize,
}

impl MappedBar {
    pub fn as_ptr(&self) -> *mut u8 {
        self.vaddr as *mut u8
    }
}

impl Drop for MappedBar {
    fn drop(&mut self) {
        let mut vaddr = self.vaddr;
        let mut size = self.info.len;
        size += vaddr % config::PAGE_SIZE as usize;
        vaddr -= vaddr % config::PAGE_SIZE as usize;
        let vpn = vaddr / config::PAGE_SIZE as usize;
        let pages = size.div_ceil(config::PAGE_SIZE as usize);
        unsafe { vmm::kernel_mm().unmap(vpn..vpn + pages) };
    }
}

/// Specialization for PCI/PCIe controller devices.
pub type PciCtlDevice = AbstractDevice<device_pcictl_t>;
impl PciCtlDevice {
    /// Get a list of devices using a filter.
    pub fn filter(filters: DeviceFilters) -> EResult<Vec<PciCtlDevice>> {
        unsafe {
            Device::filter_impl::<device_pcictl_t, PciCtlDevice, true>(
                filters,
                dev_class_t_DEV_CLASS_PCICTL,
            )
        }
    }
    /// Is a PCIe controller (as opposed to a PCI controller).
    pub fn is_pcie(&self) -> bool {
        unsafe { (*self.as_raw_ptr()).is_pcie }
    }
    /// Get the range of bus numbers; (start, end); includes both endpoints.
    pub fn bus_range(&self) -> (u8, u8) {
        unsafe {
            let ptr = self.as_raw_ptr();
            ((*ptr).bus_start, (*ptr).bus_end)
        }
    }

    /// Get a PCI function's BAR information.
    pub unsafe fn bar_info(&self, addr: PciAddr) -> [pci_bar_info_t; 6] {
        unsafe {
            let mut res = MaybeUninit::<[pci_bar_info_t; 6]>::uninit();
            raw::device_pcictl_bar_info(
                self.as_raw_ptr(),
                addr.into(),
                res.as_mut_ptr() as *mut pci_bar_info_t,
            );
            res.assume_init()
        }
    }
    /// Map a PCI function's BAR.
    /// Returns a pointer to the mapped virtual address.
    pub unsafe fn bar_map(&self, info: pci_bar_info_t) -> EResult<MappedBar> {
        unsafe {
            let res = raw::device_pcictl_bar_map(self.as_raw_ptr(), info);
            if res.is_null() {
                Err(Errno::ENOMEM)
            } else {
                Ok(MappedBar {
                    info,
                    vaddr: res as usize,
                })
            }
        }
    }
    /// Read data from the configuration space for a specific device.
    pub unsafe fn dev_cam_read<T: Sized + Copy>(
        &self,
        dev_addr: PciAddr,
        offset: u32,
        data: &mut T,
    ) {
        unsafe {
            raw::device_pcictl_dev_cam_read(
                self.as_raw_ptr(),
                dev_addr.into(),
                offset,
                size_of::<T>() as u32,
                data as *mut T as *mut core::ffi::c_void,
            );
        }
    }
    /// Write data to the configuration space for a specific device.
    pub unsafe fn dev_cam_write<T: Sized + Copy>(&self, dev_addr: PciAddr, offset: u32, data: &T) {
        unsafe {
            raw::device_pcictl_dev_cam_write(
                self.as_raw_ptr(),
                dev_addr.into(),
                offset,
                size_of::<T>() as u32,
                data as *const T as *const core::ffi::c_void,
            );
        }
    }
}
