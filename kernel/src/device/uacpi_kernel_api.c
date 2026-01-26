
// SPDX-License-Identifier: MIT

#include "assertions.h"
#include "badgeros-abi/errno.h"
#include "interrupt.h"
#include "log.h"
#include "mem/vmm.h"
#include "sched/sched.h"
#include "sched/sync/mutex.h"
#include "sched/sync/sem.h"
#include "spinlock.h"
#include "time.h"
#include "uacpi/kernel_api.h"
#include "uacpi/status.h"

#include <stdatomic.h>

#include <malloc.h>

#ifdef __x86_64__
#include "cpu/x86_ioport.h"
#endif



/*
 * Open a PCI device at 'address' for reading & writing.
 *
 * The handle returned via 'out_handle' is used to perform IO on the
 * configuration space of the device.
 */
uacpi_status uacpi_kernel_pci_device_open(uacpi_pci_address address, uacpi_handle *out_handle) {
    return UACPI_STATUS_UNIMPLEMENTED;
}
void uacpi_kernel_pci_device_close(uacpi_handle) {
}

/*
 * Read & write the configuration space of a previously open PCI device.
 */
uacpi_status uacpi_kernel_pci_read8(uacpi_handle device, uacpi_size offset, uacpi_u8 *value) {
    return UACPI_STATUS_UNIMPLEMENTED;
}
uacpi_status uacpi_kernel_pci_read16(uacpi_handle device, uacpi_size offset, uacpi_u16 *value) {
    return UACPI_STATUS_UNIMPLEMENTED;
}
uacpi_status uacpi_kernel_pci_read32(uacpi_handle device, uacpi_size offset, uacpi_u32 *value) {
    return UACPI_STATUS_UNIMPLEMENTED;
}

uacpi_status uacpi_kernel_pci_write8(uacpi_handle device, uacpi_size offset, uacpi_u8 value) {
    return UACPI_STATUS_UNIMPLEMENTED;
}
uacpi_status uacpi_kernel_pci_write16(uacpi_handle device, uacpi_size offset, uacpi_u16 value) {
    return UACPI_STATUS_UNIMPLEMENTED;
}
uacpi_status uacpi_kernel_pci_write32(uacpi_handle device, uacpi_size offset, uacpi_u32 value) {
    return UACPI_STATUS_UNIMPLEMENTED;
}

/*
 * Map a SystemIO address at [base, base + len) and return a kernel-implemented
 * handle that can be used for reading and writing the IO range.
 */
uacpi_status uacpi_kernel_io_map(uacpi_io_addr base, uacpi_size len, uacpi_handle *out_handle) {
    *out_handle = (void *)(size_t)base;
    return UACPI_STATUS_OK;
}
void uacpi_kernel_io_unmap(uacpi_handle handle) {
}

/*
 * Read/Write the IO range mapped via uacpi_kernel_io_map
 * at a 0-based 'offset' within the range.
 *
 * NOTE:
 * You are NOT allowed to break e.g. a 4-byte access into four 1-byte accesses.
 * Hardware ALWAYS expects accesses to be of the exact width.
 */
#ifdef __x86_64__
uacpi_status uacpi_kernel_io_read8(uacpi_handle handle, uacpi_size offset, uacpi_u8 *out_value) {
    *out_value = inb((size_t)handle + offset);
    return UACPI_STATUS_OK;
}
uacpi_status uacpi_kernel_io_read16(uacpi_handle handle, uacpi_size offset, uacpi_u16 *out_value) {
    *out_value = inw((size_t)handle + offset);
    return UACPI_STATUS_OK;
}
uacpi_status uacpi_kernel_io_read32(uacpi_handle handle, uacpi_size offset, uacpi_u32 *out_value) {
    *out_value = ind((size_t)handle + offset);
    return UACPI_STATUS_OK;
}

uacpi_status uacpi_kernel_io_write8(uacpi_handle handle, uacpi_size offset, uacpi_u8 in_value) {
    outb((size_t)handle + offset, in_value);
    return UACPI_STATUS_OK;
}
uacpi_status uacpi_kernel_io_write16(uacpi_handle handle, uacpi_size offset, uacpi_u16 in_value) {
    outw((size_t)handle + offset, in_value);
    return UACPI_STATUS_OK;
}
uacpi_status uacpi_kernel_io_write32(uacpi_handle handle, uacpi_size offset, uacpi_u32 in_value) {
    outd((size_t)handle + offset, in_value);
    return UACPI_STATUS_OK;
}
#else
uacpi_status uacpi_kernel_io_read8(uacpi_handle handle, uacpi_size offset, uacpi_u8 *out_value) {
    return UACPI_STATUS_UNIMPLEMENTED;
}
uacpi_status uacpi_kernel_io_read16(uacpi_handle handle, uacpi_size offset, uacpi_u16 *out_value) {
    return UACPI_STATUS_UNIMPLEMENTED;
}
uacpi_status uacpi_kernel_io_read32(uacpi_handle handle, uacpi_size offset, uacpi_u32 *out_value) {
    return UACPI_STATUS_UNIMPLEMENTED;
}

uacpi_status uacpi_kernel_io_write8(uacpi_handle handle, uacpi_size offset, uacpi_u8 in_value) {
    return UACPI_STATUS_UNIMPLEMENTED;
}
uacpi_status uacpi_kernel_io_write16(uacpi_handle handle, uacpi_size offset, uacpi_u16 in_value) {
    return UACPI_STATUS_UNIMPLEMENTED;
}
uacpi_status uacpi_kernel_io_write32(uacpi_handle handle, uacpi_size offset, uacpi_u32 in_value) {
    return UACPI_STATUS_UNIMPLEMENTED;
}
#endif

void *uacpi_kernel_map(uacpi_phys_addr paddr, uacpi_size len) {
    size_t off  = paddr % CONFIG_PAGE_SIZE;
    len        += paddr % CONFIG_PAGE_SIZE;
    paddr      -= paddr % CONFIG_PAGE_SIZE;
    if (len % CONFIG_PAGE_SIZE) {
        len += CONFIG_PAGE_SIZE - len % CONFIG_PAGE_SIZE;
    }
    vpn_t vpn;
    if (vmm_map_k(&vpn, len / CONFIG_PAGE_SIZE, paddr / CONFIG_PAGE_SIZE, VMM_FLAG_RW | VMM_FLAG_IO | VMM_FLAG_MMIO) <
        0) {
        return NULL;
    }
    return (void *)(vpn * CONFIG_PAGE_SIZE + off);
}
void uacpi_kernel_unmap(void *addr, uacpi_size len) {
    size_t vaddr  = (size_t)addr;
    len          += vaddr % CONFIG_PAGE_SIZE;
    vaddr        -= vaddr % CONFIG_PAGE_SIZE;
    if (len % CONFIG_PAGE_SIZE) {
        len += CONFIG_PAGE_SIZE - len % CONFIG_PAGE_SIZE;
    }
    vmm_unmap_k(vaddr / CONFIG_PAGE_SIZE, len / CONFIG_PAGE_SIZE);
}

/*
 * Allocate a block of memory of 'size' bytes.
 * The contents of the allocated memory are unspecified.
 */
void *uacpi_kernel_alloc(uacpi_size size) {
    return malloc(size);
}

/*
 * Allocate a block of memory of 'count' * 'size' bytes.
 * The returned memory block is expected to be zero-filled.
 */
void *uacpi_kernel_calloc(uacpi_size count, uacpi_size size) {
    return calloc(count, size);
}

/*
 * Free a previously allocated memory block.
 *
 * 'mem' might be a NULL pointer. In this case, the call is assumed to be a
 * no-op.
 *
 * An optionally enabled 'size_hint' parameter contains the size of the original
 * allocation. Note that in some scenarios this incurs additional cost to
 * calculate the object size.
 */
void uacpi_kernel_free(void *mem) {
    free(mem);
}

/*
 * No description provided.
 */
void uacpi_kernel_log(uacpi_log_level uacpi_level, uacpi_char const *msg) {
    log_level_t level;
    switch (uacpi_level) {
        case UACPI_LOG_DEBUG:
        case UACPI_LOG_TRACE: level = LOG_DEBUG; break;
        case UACPI_LOG_INFO: level = LOG_INFO; break;
        case UACPI_LOG_WARN: level = LOG_WARN; break;
        case UACPI_LOG_ERROR: level = LOG_ERROR; break;
        default: level = LOG_INFO; break;
    }
    logk(level, msg);
}

/*
 * Returns the number of nanosecond ticks elapsed since boot,
 * strictly monotonic.
 */
uacpi_u64 uacpi_kernel_get_nanoseconds_since_boot(void) {
    return time_us() * 1000;
}

/*
 * Spin for N microseconds.
 */
void uacpi_kernel_stall(uacpi_u8 usec) {
    timestamp_us_t lim = time_us() + usec;
    while (time_us() < lim);
}

/*
 * Sleep for N milliseconds.
 */
void uacpi_kernel_sleep(uacpi_u64 msec) {
    thread_sleep((int64_t)msec * 1000);
}

/*
 * Create/free an opaque non-recursive kernel mutex object.
 */
uacpi_handle uacpi_kernel_create_mutex(void) {
    mutex_t *mutex = calloc(1, sizeof(mutex_t));
    if (!mutex)
        return NULL;
    *mutex = MUTEX_T_INIT;
    atomic_store(&mutex->shares, 0); // Causes release fence.
    return mutex;
}

void uacpi_kernel_free_mutex(uacpi_handle handle) {
    mutex_t *mutex = handle;
    assert_dev_drop(atomic_load(&mutex->shares) == 0);
    free(handle);
}

/*
 * Create/free an opaque kernel (semaphore-like) event object.
 */
uacpi_handle uacpi_kernel_create_event() {
    sem_t *sem = malloc(sizeof(sem_t));
    *sem       = SEM_T_INIT;
    atomic_store(&sem->counter, 0); // Causes release fence.
    return sem;
}

void uacpi_kernel_free_event(uacpi_handle handle) {
    sem_t *sem = handle;
    assert_dev_drop(atomic_load(&sem->counter) == 0);
    free(sem);
}

/*
 * Returns a unique identifier of the currently executing thread.
 *
 * The returned thread id cannot be UACPI_THREAD_ID_NONE.
 */
uacpi_thread_id uacpi_kernel_get_thread_id(void) {
    return (uacpi_thread_id)thread_current();
}

/*
 * Try to acquire the mutex with a millisecond timeout.
 *
 * The timeout value has the following meanings:
 * 0x0000 - Attempt to acquire the mutex once, in a non-blocking manner
 * 0x0001...0xFFFE - Attempt to acquire the mutex for at least 'timeout'
 *                   milliseconds
 * 0xFFFF - Infinite wait, block until the mutex is acquired
 *
 * The following are possible return values:
 * 1. UACPI_STATUS_OK - successful acquire operation
 * 2. UACPI_STATUS_TIMEOUT - timeout reached while attempting to acquire (or the
 *                           single attempt to acquire was not successful for
 *                           calls with timeout=0)
 * 3. Any other value - signifies a host internal error and is treated as such
 */
uacpi_status uacpi_kernel_acquire_mutex(uacpi_handle handle, uacpi_u16 timeout0) {
    timestamp_us_t timeout = timeout0 == 0xffff ? TIMESTAMP_US_MAX : (timestamp_us_t)timeout0 * 1000;
    errno_t        res     = mutex_timed_lock(handle, timeout);
    if (res >= 0) {
        return UACPI_STATUS_OK;
    } else if (res == -ETIMEDOUT) {
        return UACPI_STATUS_TIMEOUT;
    } else {
        return UACPI_STATUS_INTERNAL_ERROR;
    }
}

void uacpi_kernel_release_mutex(uacpi_handle handle) {
    mutex_unlock(handle);
}

/*
 * Try to wait for an event (counter > 0) with a millisecond timeout.
 * A timeout value of 0xFFFF implies infinite wait.
 *
 * The internal counter is decremented by 1 if wait was successful.
 *
 * A successful wait is indicated by returning UACPI_TRUE.
 */
uacpi_bool uacpi_kernel_wait_for_event(uacpi_handle handle, uacpi_u16 timeout0) {
    timestamp_us_t timeout = timeout0 == 0xffff ? TIMESTAMP_US_MAX : (timestamp_us_t)timeout0 * 1000;
    return sem_timed_wait(handle, timeout) >= 0;
}

/*
 * Signal the event object by incrementing its internal counter by 1.
 *
 * This function may be used in interrupt contexts.
 */
void uacpi_kernel_signal_event(uacpi_handle handle) {
    sem_post(handle);
}

/*
 * Reset the event counter to 0.
 */
void uacpi_kernel_reset_event(uacpi_handle handle) {
    sem_t *sem = handle;
    atomic_store(&sem->counter, 0);
}

/*
 * Handle a firmware request.
 *
 * Currently either a Breakpoint or Fatal operators.
 */
uacpi_status uacpi_kernel_handle_firmware_request(uacpi_firmware_request *req) {
    switch (req->type) {
        case UACPI_FIRMWARE_REQUEST_TYPE_BREAKPOINT: __builtin_trap(); break;
        case UACPI_FIRMWARE_REQUEST_TYPE_FATAL:
            logk(LOG_FATAL, "AML panicked");
            panic_abort();
            break;
        default:
            logkf(LOG_FATAL, "AML requested unsupported firmware operation %{u8;d}", req->type);
            panic_abort();
            break;
    }
}

/*
 * Install an interrupt handler at 'irq', 'ctx' is passed to the provided
 * handler for every invocation.
 *
 * 'out_irq_handle' is set to a kernel-implemented value that can be used to
 * refer to this handler from other API.
 */
uacpi_status uacpi_kernel_install_interrupt_handler(
    uacpi_u32 irq, uacpi_interrupt_handler isr, uacpi_handle ctx, uacpi_handle *out_irq_handle
) {
    return UACPI_STATUS_UNIMPLEMENTED;
}

/*
 * Uninstall an interrupt handler. 'irq_handle' is the value returned via
 * 'out_irq_handle' during installation.
 */
uacpi_status uacpi_kernel_uninstall_interrupt_handler(uacpi_interrupt_handler isr, uacpi_handle irq_handle) {
    return UACPI_STATUS_UNIMPLEMENTED;
}

/*
 * Create/free a kernel spinlock object.
 *
 * Unlike other types of locks, spinlocks may be used in interrupt contexts.
 */
uacpi_handle uacpi_kernel_create_spinlock(void) {
    spinlock_t *lock = malloc(sizeof(spinlock_t));
    if (!lock)
        return NULL;
    *lock = SPINLOCK_T_INIT;
    atomic_thread_fence(memory_order_release);
    return lock;
}
void uacpi_kernel_free_spinlock(uacpi_handle handle) {
    free(handle);
}

/*
 * Lock/unlock helpers for spinlocks.
 *
 * These are expected to disable interrupts, returning the previous state of cpu
 * flags, that can be used to possibly re-enable interrupts if they were enabled
 * before.
 *
 * Note that lock is infalliable.
 */
uacpi_cpu_flags uacpi_kernel_lock_spinlock(uacpi_handle handle) {
    bool ie = irq_disable();
    spinlock_take(handle);
    return ie;
}

void uacpi_kernel_unlock_spinlock(uacpi_handle handle, uacpi_cpu_flags flags) {
    spinlock_release(handle);
    irq_enable_if(flags);
}



/*
 * Schedules deferred work for execution.
 * Might be invoked from an interrupt context.
 */
uacpi_status uacpi_kernel_schedule_work(uacpi_work_type type, uacpi_work_handler handler, uacpi_handle ctx) {
    return UACPI_STATUS_UNIMPLEMENTED;
}

/*
 * Waits for two types of work to finish:
 * 1. All in-flight interrupts installed via uacpi_kernel_install_interrupt_handler
 * 2. All work scheduled via uacpi_kernel_schedule_work
 *
 * Note that the waits must be done in this order specifically.
 */
uacpi_status uacpi_kernel_wait_for_work_completion(void) {
    return UACPI_STATUS_OK;
}
