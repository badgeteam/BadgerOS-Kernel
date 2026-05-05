// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::bindings::error::EResult;

/// Describes at what level of init a testcase should run.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KTestWhen {
    /// Run after `bootp_early_init`.
    Early,
    /// Run after PMM is set up.
    PMM,
    /// Run after VMM is set up.
    VMM,
    /// Run after `kernel_heap_init`.
    Heap,
    /// Run when the scheduler has started.
    Sched,
    /// Run when filesystem is mounted.
    RootFs,
}

/// Tests table entry.
#[repr(C)]
pub struct KTest {
    pub when: KTestWhen,
    pub name: &'static str,
    pub func: &'static fn() -> EResult<()>,
}

/// Run test cases of a certain level.
#[unsafe(no_mangle)]
#[cfg(feature = "ktest")]
pub extern "C" fn ktests_runlevel(level: KTestWhen) {
    unsafe extern "C" {
        static __start_ktests: u8;
        static __stop_ktests: u8;
    }
    let tests = unsafe {
        use core::ptr::slice_from_raw_parts;

        let start = &raw const __start_ktests as *const KTest;
        let stop = &raw const __stop_ktests as *const KTest;
        &*slice_from_raw_parts(start, stop.offset_from_unsigned(start))
    };
    printf!(
        "Running tests for level {}\n",
        match level {
            KTestWhen::Early => "Early",
            KTestWhen::PMM => "PMM",
            KTestWhen::VMM => "VMM",
            KTestWhen::Heap => "Heap",
            KTestWhen::Sched => "Sched",
            KTestWhen::RootFs => "RootFs",
        }
    );
    let mut total = 0u32;
    let mut success = 0u32;
    for test in tests {
        if test.when != level {
            continue;
        }
        total += 1;
        printf!("Test {}...", test.name);
        match (test.func)() {
            Ok(()) => {
                printf!(" \x1b[32mOK\x1b[0m\n");
                success += 1;
            }
            Err(x) => printf!(" \x1b[31m{}\x1b[0m\n", x),
        }
    }
    printf!(
        "{} / {} ({}%) tests passed\n",
        success,
        total,
        if total == 0 {
            100
        } else {
            (success * 100) / total
        }
    );
}

/// Dummy function; runs test cases of a certain level if ktest feature is enabled.
#[unsafe(no_mangle)]
#[cfg(not(feature = "ktest"))]
pub fn ktests_runlevel(_level: KTestWhen) {}

/// Register a kernel test case to run after `bootp_early_init`.
#[macro_export]
macro_rules! early_ktest {
    ($name: ident, $($code: tt)*) => {
        crate::ktest!(crate::ktest::KTestWhen::Early, $name, $($code)*);
    };
}

/// Register a kernel test case to run after PMM is set up.
#[macro_export]
macro_rules! pmm_ktest {
    ($name: ident, $($code: tt)*) => {
        crate::ktest!(crate::ktest::KTestWhen::PMM, $name, $($code)*);
    };
}

/// Register a kernel test case to run after VMM is set up.
#[macro_export]
macro_rules! vmm_ktest {
    ($name: ident, $($code: tt)*) => {
        crate::ktest!(crate::ktest::KTestWhen::VMM, $name, $($code)*);
    };
}

/// Register a kernel test case to run after `kernel_heap_init`.
#[macro_export]
macro_rules! heap_ktest {
    ($name: ident, $($code: tt)*) => {
        crate::ktest!(crate::ktest::KTestWhen::Heap, $name, $($code)*);
    };
}

/// Register a kernel test case to run when the scheduler has started.
#[macro_export]
macro_rules! sched_ktest {
    ($name: ident, $($code: tt)*) => {
        crate::ktest!(crate::ktest::KTestWhen::Sched, $name, $($code)*);
    };
}

/// Register a kernel test case to run when filesystem is mounted.
#[macro_export]
macro_rules! rootfs_ktest {
    ($name: ident, $($code: tt)*) => {
        crate::ktest!(crate::ktest::KTestWhen::RootFs, $name, $($code)*);
    };
}

/// Register a kernel test case.
#[macro_export]
#[cfg(feature = "ktest")]
macro_rules! ktest {
    ($when: expr, $name: ident, $($code: tt)*) => {
        #[used]
        #[unsafe(link_section = ".ktests")]
        static $name: crate::ktest::KTest = crate::ktest::KTest {
            when: $when,
            name: stringify!($name),
            func: {
                fn func() -> crate::bindings::error::EResult<()> {
                    {$($code)*}
                    Ok(())
                }
                &(func as fn() -> crate::bindings::error::EResult<()>)
            },
        };
    };
}

/// Dummy macro; registers tests if ktest feature is enabled.
#[macro_export]
#[cfg(not(feature = "ktest"))]
macro_rules! ktest {
    ($when: expr, $name: ident, $($code: tt)*) => {};
}

/// Assert that some expression has a certain relation.
#[macro_export]
macro_rules! ktest_expect {
    ($lhs: expr, $oper: tt, $rhs: expr $(, [ $($ctx: expr),* ])?) => {
        let lhs = $lhs;
        let rhs = $rhs;
        if !(lhs $oper rhs) {
            crate::printf!(
                "\n\x1b[31mExpected\x1b[0m {} {} {}\x1b[31m, got\x1b[0m {:?} \x1b[31mand\x1b[0m {:?}\n",
                stringify!($lhs), stringify!($oper), stringify!($rhs), {lhs}, {rhs}
            );
            $($(
                crate::printf!("    where {} => {}\n", stringify!($ctx), {$ctx});
            )*)?
            Err(crate::bindings::error::Errno::EASSERT)?;
        }
    };
    ($lhs: expr, $rhs: expr $(, [ $($ctx: expr),* ])?) => {
        crate::ktest_expect!($lhs, ==, $rhs $(, [ $($ctx),* ])?);
    };
}

/// Assert that some condition is satisfiead.
#[macro_export]
macro_rules! ktest_assert {
    ($cond: expr $(, [ $($ctx: expr),* ])?) => {
        if !$cond {
            crate::printf!(
                "\n\x1b[31mAssertion\x1b[0m {} \x1b[31mfailed\x1b[0m\n",
                stringify!($cond)
            );
            $($(
                crate::printf!("    where {} => {:?}\n", stringify!($ctx), {$ctx});
            )*)?
            Err(crate::bindings::error::Errno::EASSERT)?;
        }
    };
}
