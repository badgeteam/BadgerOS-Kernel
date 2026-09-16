// This file is almost exclusively interfacing with uacpi_sys and unsafe code.
#![allow(unsafe_op_in_unsafe_fn)]
// Until the entire file is implemented.
#![allow(unused)]

use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::{CStr, c_char, c_void},
    ptr::null_mut,
};

use alloc::{boxed::Box, sync::Arc};

use crate::{
    LogLevel,
    arch::{self, Arch, except::ArchExcept},
    boot,
    error::Errno,
    kcore::{
        sched::{Thread, thread_sleep},
        sync::{
            mutex::{RawMutex, RawMutexGuard},
            semaphore::Semaphore,
            spinlock::{RawSpinlock, RawSpinlockGuard},
        },
        timer::time_us,
    },
    mem::{
        heap::HEAP,
        pmm::{self, PAddrr},
        vmm::{self, map::Mapping, memobject::RawMemory},
    },
    uacpi_sys::*,
};

#[unsafe(no_mangle)]
pub extern "C" fn uacpi_kernel_get_rsdp(out_rsdp: &mut uacpi_phys_addr) -> uacpi_status {
    let paddr = boot::protocol::get_rsdp_paddr();
    assert!(paddr != 0); // We checked this earlier already.
    *out_rsdp = paddr as _;
    UACPI_STATUS_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uacpi_kernel_map(addr: uacpi_phys_addr, len: uacpi_size) -> *mut c_void {
    let page_start = addr as PAddrr / arch::mmu::PAGE_SIZE;
    let io;
    if pmm::page_range().contains(&page_start) {
        match (*pmm::page_struct(page_start * arch::mmu::PAGE_SIZE)).usage() {
            pmm::PageUsage::Unusable => io = vmm::prot::IO, // Hole in memmap => assume I/O.
            _ => io = 0, // Accounted for in memmap => assume RAM.
        }
    } else {
        io = vmm::prot::IO; // Not in the accounted range => assume I/O.
    }

    match vmm::kernel_mm().map(
        len,
        0,
        vmm::map::SHARED,
        vmm::prot::READ | vmm::prot::WRITE | io,
        Some(Mapping {
            offset: 0,
            object: Arc::new(RawMemory::new(addr as _, len)),
        }),
    ) {
        Ok(vma) => vma as *mut c_void,
        Err(x) => {
            panic!("uacpi_kernel_map: {}", x);
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_unmap(addr: *mut c_void, len: uacpi_size) {
    let addr = addr as usize;
    vmm::kernel_mm()
        .unmap(addr..addr + len)
        .expect("Invalid uacpi_kernel_unmap");
}

// TODO: Import nanoprintf for our troubles here.
#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_log(level: uacpi_log_level, msg: *const c_char) {
    let msg = CStr::from_ptr(msg).to_str().unwrap();
    let level = match level {
        UACPI_LOG_DEBUG => LogLevel::Debug,
        UACPI_LOG_ERROR => LogLevel::Error,
        UACPI_LOG_INFO => LogLevel::Info,
        UACPI_LOG_TRACE => LogLevel::Debug,
        UACPI_LOG_WARN => LogLevel::Warning,
        _ => LogLevel::Info,
    };
    logkf!(level, "{}", msg.trim_ascii());
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_pci_device_open(
    addr: uacpi_pci_address,
    handle: *mut uacpi_handle,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_pci_device_close(handle: uacpi_handle) {
    todo!()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_pci_read8(
    device: uacpi_handle,
    offset: uacpi_size,
    value: &mut u8,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_pci_read16(
    device: uacpi_handle,
    offset: uacpi_size,
    value: &mut u16,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_pci_read32(
    device: uacpi_handle,
    offset: uacpi_size,
    value: &mut u32,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_pci_write8(
    device: uacpi_handle,
    offset: uacpi_size,
    value: u8,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_pci_write16(
    device: uacpi_handle,
    offset: uacpi_size,
    value: u16,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_pci_write32(
    device: uacpi_handle,
    offset: uacpi_size,
    value: u32,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_io_map(
    base: uacpi_io_addr,
    len: uacpi_size,
    handle: *mut uacpi_handle,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_io_unmap(handle: uacpi_handle) {
    todo!()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_io_read8(
    device: uacpi_handle,
    offset: uacpi_size,
    value: &mut u8,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_io_read16(
    device: uacpi_handle,
    offset: uacpi_size,
    value: &mut u16,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_io_read32(
    device: uacpi_handle,
    offset: uacpi_size,
    value: &mut u32,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_io_write8(
    device: uacpi_handle,
    offset: uacpi_size,
    value: u8,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_io_write16(
    device: uacpi_handle,
    offset: uacpi_size,
    value: u16,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_io_write32(
    device: uacpi_handle,
    offset: uacpi_size,
    value: u32,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_alloc(size: uacpi_size) -> *mut c_void {
    HEAP.alloc(Layout::from_size_align_unchecked(size, 1))
        .cast()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_free(mem: *mut c_void, size_hint: uacpi_size) {
    HEAP.dealloc(mem.cast(), Layout::from_size_align_unchecked(size_hint, 1));
}

#[unsafe(no_mangle)]
extern "C" fn uacpi_kernel_get_nanoseconds_since_boot() -> u64 {
    time_us() * 1000
}

#[unsafe(no_mangle)]
extern "C" fn uacpi_kernel_stall(usec: u8) {
    let now = time_us();
    if now > 0 {
        let lim = now + usec as u64;
        while time_us() < lim {}
    }
}

#[unsafe(no_mangle)]
extern "C" fn uacpi_kernel_sleep(msec: u64) {
    let _ = thread_sleep(msec * 1000);
}

#[unsafe(no_mangle)]
extern "C" fn uacpi_kernel_create_mutex() -> uacpi_handle {
    Box::into_raw(Box::new(RawMutex::new())) as uacpi_handle
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_free_mutex(mutex: uacpi_handle) {
    drop(Box::from_raw(mutex as *mut RawMutex));
}

#[unsafe(no_mangle)]
extern "C" fn uacpi_kernel_create_event() -> uacpi_handle {
    Box::into_raw(Box::new(Semaphore::new())) as uacpi_handle
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_free_event(mutex: uacpi_handle) {
    drop(Box::from_raw(mutex as *mut Semaphore));
}

#[unsafe(no_mangle)]
extern "C" fn uacpi_kernel_get_thread_id() -> uacpi_thread_id {
    Thread::current() as *mut c_void
}

#[unsafe(no_mangle)]
extern "C" fn uacpi_kernel_disable_interrupts() -> uacpi_interrupt_state {
    Arch::get_disable_irq() as u64
}

#[unsafe(no_mangle)]
extern "C" fn uacpi_kernel_restore_interrupts(state: uacpi_interrupt_state) {
    Arch::enable_irq_if(state != 0);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_acquire_mutex(
    handle: uacpi_handle,
    timeout: u16,
) -> uacpi_status {
    let handle = &*(handle as *const RawMutex);
    if timeout == u16::MAX {
        core::mem::forget(handle.unintr_lock());
        UACPI_STATUS_OK
    } else {
        match handle.unintr_timed_lock(timeout as u64 * 1000) {
            Ok(guard) => {
                core::mem::forget(guard);
                UACPI_STATUS_OK
            }
            Err(Errno::ETIMEDOUT) => UACPI_STATUS_TIMEOUT,
            Err(_) => UACPI_STATUS_INTERNAL_ERROR,
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_release_mutex(handle: uacpi_handle) {
    drop(RawMutexGuard::from_raw(&*(handle as *const RawMutex)));
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_wait_for_event(handle: uacpi_handle, timeout: u16) -> bool {
    let handle = &*(handle as *const Semaphore);
    if timeout == u16::MAX {
        handle.unintr_wait();
        true
    } else {
        handle.unintr_timed_wait(timeout as u64 * 1000).is_ok()
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_signal_event(handle: uacpi_handle) {
    let handle = &*(handle as *const Semaphore);
    handle.post();
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_reset_event(handle: uacpi_handle) {
    let handle = &*(handle as *const Semaphore);
    handle.reset();
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_handle_firmware_request(request: *const uacpi_firmware_request) {
    todo!()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_install_interrupt_handler(
    irq: u32,
    handler: uacpi_interrupt_handler,
    ctx: uacpi_handle,
    out_irq_handle: *mut uacpi_handle,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_uninstall_interrupt_handler(
    handler: uacpi_interrupt_handler,
    irq_handle: uacpi_handle,
) {
    todo!()
}

#[unsafe(no_mangle)]
extern "C" fn uacpi_kernel_create_spinlock() -> uacpi_handle {
    Box::into_raw(Box::new(RawSpinlock::new())) as uacpi_handle
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_free_spinlock(spinlock: uacpi_handle) {
    drop(Box::from_raw(spinlock as *mut RawSpinlock));
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_lock_spinlock(handle: uacpi_handle) -> uacpi_cpu_flags {
    let handle = &*(handle as *const RawSpinlock);
    let irq = Arch::get_disable_irq();
    core::mem::forget(handle.lock());
    irq as uacpi_cpu_flags
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_unlock_spinlock(handle: uacpi_handle, flags: uacpi_cpu_flags) {
    let handle = &*(handle as *const RawSpinlock);
    drop(RawSpinlockGuard::from_raw(handle));
    Arch::enable_irq_if(flags != 0);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_schedule_work(
    type_: uacpi_work_type,
    handler: uacpi_work_handler,
    ctx: uacpi_handle,
) -> uacpi_status {
    UACPI_STATUS_UNIMPLEMENTED
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uacpi_kernel_wait_for_work_completion() -> uacpi_status {
    UACPI_STATUS_OK
}
