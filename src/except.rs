use crate::{
    arch::except::{ArchTrapFrame, TrapCause, TrapFrame},
    bindings::log::LogLevel,
    kcore::sched::Thread,
    mem::vmm::{self, kernel_mm},
    misc::panic::unhandled_trap,
    process::uapi::{signal::Signal, wait::w_signalled},
};

/// Entry in the `.noexc_table` table.
#[derive(Clone, Copy)]
#[repr(C)]
struct NoexcEntry {
    start: *const (),
    end: *const (),
}

unsafe extern "C" {
    static __start_noexc: NoexcEntry;
    static __stop_noexc: NoexcEntry;
}

/// Try to handle demand-paging.
/// Returns `true` if the access should be retried.
fn check_demand_paging(vaddr: usize, access: u8) -> bool {
    let current = Thread::current();
    if current.is_null() {
        return false;
    }

    let mm = unsafe { (*current).runtime().memmap };
    if mm.is_null() {
        kernel_mm().fault(vaddr, access, 1).is_ok()
    } else {
        unsafe { (*mm).fault(vaddr, access, 1).is_ok() }
    }
}

/// Generic exception handler.
pub fn generic_trap(frame: &mut TrapFrame) {
    let Some(cause) = frame.get_cause() else {
        unhandled_trap(frame);
    };

    let demand_paging_ok = match cause {
        TrapCause::PageFaultLoad => check_demand_paging(frame.get_addr().unwrap(), vmm::prot::READ),
        TrapCause::PageFaultStore => {
            check_demand_paging(frame.get_addr().unwrap(), vmm::prot::WRITE)
        }
        TrapCause::PageFaultExec => check_demand_paging(frame.get_addr().unwrap(), vmm::prot::EXEC),
        _ => false,
    };
    if demand_paging_ok {
        return;
    }

    if frame.is_kernel_mode() {
        // Check noexc table.
        let pc = frame.get_pc();
        let mut cur = &raw const __start_noexc;
        while !core::ptr::addr_eq(cur, &raw const __stop_noexc) {
            let entry = unsafe { *cur };
            if entry.start <= pc && pc < entry.end {
                frame.noexc_skip(entry.end);
                return;
            }
            cur = cur.wrapping_add(1);
        }

        // Unhandled kernel trap, this is fatal.
        unhandled_trap(frame);
    } else {
        let current = Thread::current();
        assert!(
            !current.is_null(),
            "User-mode trap without associated thread"
        );
        let current = unsafe { &*current };
        let proc = current
            .process
            .as_deref()
            .expect("User-mode trap without associated process");

        // TODO: Raise signal to process.

        // Unable to correctly handle this fault, we must kill the process.
        logkf!(
            LogLevel::Error,
            "Oops: Unhandled exception {}",
            frame.get_number()
        );
        printf!(
            "**** BEGIN OOPS DUMP ****\n{}**** END OOPS DUMP ****\n",
            frame
        );
        proc.die(w_signalled(Signal::SIGBUS as i32)); // W_SIGNALLED
        unsafe { current.die() };
    }
}
