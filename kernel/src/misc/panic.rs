// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

use crate::{
    bindings::log::write_unlocked,
    cpu::{
        backtrace::{backtrace, get_frame_ptr},
        panic::panic_cpu_shutdown,
        thread::{GpRegfile, SpRegfile},
    },
};

static IS_PANICKING: AtomicU32 = AtomicU32::new(0);

/// Panic due to an unhandled exception.
pub fn unhandled_trap(regs: &GpRegfile, sregs: &SpRegfile) -> ! {
    claim_panic();

    printf_unlocked!(
        "\x1b[0m\n\n**** UNHANDLED EXCEPTION 0x{:x} ****\n",
        sregs.fault_code()
    );
    if let Some(name) = sregs.fault_name() {
        printf_unlocked!("{}\n", name);
    }
    if sregs.is_kernel_mode() {
        write_unlocked("Running in kernel mode\n");
    } else {
        write_unlocked("Running in user mode\n");
    }
    if let Some(vaddr) = sregs.is_mem_trap() {
        printf_unlocked!("While accessing 0x{:x}\n", vaddr);
    }

    backtrace(regs.s0 as *const ());

    printf_unlocked!(
        "**** BEGIN REGISTER DUMP ****\n{}{}**** END REGISTER DUMP ****\n",
        regs,
        sregs
    );

    write_unlocked("**** KERNEL PANIC ****\n");
    kekw();

    panic_cpu_shutdown();
}

/// Generic kernel panic.
#[unsafe(no_mangle)]
pub extern "C" fn kernel_panic() -> ! {
    claim_panic();
    kernel_panic_unchecked();
}

/// Generic kernel panic without checking for other cores panicking.
pub fn kernel_panic_unchecked() -> ! {
    write_unlocked("\x1b[0m\n\n");

    backtrace(get_frame_ptr());

    write_unlocked("**** KERNEL PANIC ****\n");
    kekw();

    panic_cpu_shutdown();
}

#[unsafe(no_mangle)]
unsafe extern "C" fn panic_abort() -> ! {
    kernel_panic();
}

#[unsafe(no_mangle)]
unsafe extern "C" fn abort() -> ! {
    kernel_panic();
}

#[unsafe(no_mangle)]
unsafe extern "C" fn panic_abort_unchecked() -> ! {
    kernel_panic_unchecked();
}

#[unsafe(no_mangle)]
unsafe extern "C" fn panic_poweroff() -> ! {
    panic_cpu_shutdown();
}

/// Checks whether other cores are panicking and spins if they do.
pub fn check_for_panic() {
    if IS_PANICKING.load(Ordering::Relaxed) != 0 {
        panic_cpu_shutdown();
    }
}

/// Start the process of kernel panicking.
/// Checks whether other cores are panicking and spin early if they do.
/// If no other core has panicked, returns and assumes the caller will eventually call [`kernel_panic_unlocked`].
#[unsafe(no_mangle)]
pub extern "C" fn claim_panic() {
    if IS_PANICKING.fetch_add(1, Ordering::Relaxed) != 0 {
        panic_cpu_shutdown();
    }
}

pub fn kekw() {
    let msg = concat!(
        "======+++++++***************####**++++++========\n",
        "=--:::----:-==++*****+++++==++++*+====---=======\n",
        "-::........::-==++++++===--:::.:::::::-=========\n",
        ":::----=---:::-====++===--:::...::-=============\n",
        "--==+++++=+++=::--==+++=----==+++++***+++=======\n",
        ":.      :----======+#*++===-===---:.:::::--=====\n",
        "=----===+++++======+**++++=====--::-===---------\n",
        "==----:-==========++++++++++++====++++**++====++\n",
        "========+++========+++++++++++++++=======+++=+++\n",
        "=====++++++========++++====+++***++++++**#*+++++\n",
        "=====++++++=======++====-=====+*##******##*+++++\n",
        "===+++++=======+++**+==-=========*#######*++++++\n",
        "=========-===---========+++=--=*+==+****++++++++\n",
        "---====--==:...:----::.  .::::=========+++======\n",
        "-------:--:..........:::::::::::::-=--==========\n",
        "--------:. .. ....:-:. .::...:::..::----========\n",
        "-------:...........--....::...::...::::---======\n",
        "------:. .........-===:.:::...:::......:---=====\n",
        "-----=-. .... ..     ..........:::::::.  :--====\n",
        "------=-::-...+##=                    ::-:-=====\n",
        "::::--====-=+:     :::......:--=----:.-----====-\n",
        ".::----==--=+=--=+++**********++==---===--===---\n",
        ".:-:-=--===-==--=+****++++++++++=--=*===-====---\n",
        "..:-:==-======---=++++++++====---===+========---\n",
        "..:---==-=====---==========+#*=--==+++=======--=\n",
        "...--:=+===---============++++=====++=======---=\n",
    );
    // c9 8d 74
    write_unlocked("\x1b[38;2;201;141;116m\n\n");
    write_unlocked(msg);
    write_unlocked("\x1b[0m\n\n");
}
