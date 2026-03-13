
// SPDX-License-Identifier: MIT

#pragma once

#include "attributes.h"

// Try to atomically claim the panic flag.
// Only ever call this if a subsequent call to `panic_abort_unchecked` or `panic_poweroff_unchecked` is imminent.
void claim_panic();

// Like `panic_abort`, but does not check the panic flag.
void panic_abort_unchecked() NORETURN;
// Like `panic_poweroff`, but does not check the panic flag.
void panic_poweroff_unchecked() NORETURN;

// Call this function when and only when the kernel has encountered a fatal error.
// Prints register dump for current kernel context and jumps to `panic_poweroff`.
void panic_abort() NORETURN;
// Call this function when and only when the kernel has encountered a fatal error.
// Immediately power off or reset the system.
void panic_poweroff() NORETURN;
// Check for a panic and immediately halt if it has happened.
void check_for_panic();
