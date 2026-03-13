// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::AtomicU32;

use crate::{mem::vmm::Memmap, process::PID};

use super::Process;

/// Needed by C because the process struct is not representable in C.
#[unsafe(no_mangle)]
unsafe extern "C" fn proc_memmap(proc: &Process) -> &Memmap {
    proc.memmap()
}

/// Needed by C because the process struct is not representable in C.
#[unsafe(no_mangle)]
unsafe extern "C" fn proc_flags(proc: &Process) -> &AtomicU32 {
    &proc.flags
}

/// Needed by C because the process struct is not representable in C.
#[unsafe(no_mangle)]
unsafe extern "C" fn proc_pid(proc: &Process) -> PID {
    proc.pid
}

/// Start the init process.
#[unsafe(no_mangle)]
unsafe extern "C" fn proc_start_init() {
    super::Process::new_init().expect("Failed to start init process");
}
