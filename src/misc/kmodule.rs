// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::bindings::log::LogLevel;

/// Kernel module metadata.
pub struct KModule {
    pub name: &'static str,
    pub init: fn(),
}

#[macro_export]
macro_rules! register_kmodule {
    ($name:expr, $init:expr) => {
        #[used]
        #[unsafe(link_section = ".kmodules")]
        static KMODULE_TABLE_ENTRY: &'static crate::misc::kmodule::KModule =
            &crate::misc::kmodule::KModule {
                name: $name,
                init: $init as fn(),
            };
    };
}

pub unsafe fn init_builtins() {
    #[allow(improper_ctypes)]
    unsafe extern "C" {
        static __start_kmodules: &'static KModule;
        static __stop_kmodules: &'static KModule;
    }
    let mut cur = &raw const __start_kmodules;
    let end = &raw const __stop_kmodules;

    while cur != end {
        unsafe {
            logkf!(LogLevel::Info, "Init built-in module '{}'", (*cur).name);
            ((*cur).init)();
            cur = cur.add(1);
        }
    }
}
