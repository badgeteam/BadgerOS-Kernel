// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    panic::PanicInfo,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{
    arch::{
        Arch,
        except::{ArchExcept, ArchTrapFrame, TrapFrame},
        kcore::sched::ArchSched,
        misc::ArchMisc,
        usermode::ArchUsermode,
    },
    bindings::log::{LogLevel, logkf_unlocked, write_unlocked},
    mem::vmm::physmap::is_canon_addr,
    process::usercopy::AccessResult,
};

static IS_PANICKING: AtomicU32 = AtomicU32::new(0);

#[panic_handler]
#[inline(never)]
pub fn rust_panic(info: &PanicInfo) -> ! {
    claim_panic();

    if let Some(loc) = info.location() {
        logkf_unlocked!(
            LogLevel::Fatal,
            "{}:{}:{}: {}",
            loc.file(),
            loc.line(),
            loc.column(),
            info.message()
        );
    } else {
        logkf_unlocked(LogLevel::Fatal, &info.message());
    }

    kernel_panic_unchecked();
}

/// Panic due to an unhandled exception.
pub fn unhandled_trap(frame: &TrapFrame) -> ! {
    claim_panic();

    printf_unlocked!(
        "\x1b[0m\n\n**** UNHANDLED EXCEPTION 0x{:x} ****\n",
        frame.get_number()
    );
    if let Some(name) = frame.get_name() {
        printf_unlocked!("{}\n", name);
    }
    if frame.is_kernel_mode() {
        write_unlocked("Running in kernel mode\n");
    } else {
        write_unlocked("Running in user mode\n");
    }
    if let Some(vaddr) = frame.get_addr() {
        printf_unlocked!("While accessing 0x{:x}\n", vaddr);
    }

    backrtace(frame.get_frame_ptr());

    printf_unlocked!(
        "**** BEGIN REGISTER DUMP ****\n{}**** END REGISTER DUMP ****\n",
        frame
    );

    write_unlocked("**** KERNEL PANIC ****\n");
    kekw();

    panic_spin();
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

    backrtace(Arch::cur_frame_ptr());

    write_unlocked("**** KERNEL PANIC ****\n");
    kekw();

    panic_spin();
}

#[unsafe(no_mangle)]
unsafe extern "C" fn panic_abort() -> ! {
    claim_panic();
    kernel_panic_unchecked();
}

#[unsafe(no_mangle)]
unsafe extern "C" fn abort() -> ! {
    claim_panic();
    kernel_panic_unchecked();
}

#[unsafe(no_mangle)]
unsafe extern "C" fn panic_abort_unchecked() -> ! {
    kernel_panic_unchecked();
}

#[unsafe(no_mangle)]
unsafe extern "C" fn panic_poweroff() -> ! {
    panic_spin();
}

/// Checks whether other cores are panicking and spins if they do.
pub fn check_for_panic() {
    if IS_PANICKING.load(Ordering::Relaxed) != 0 {
        panic_spin();
    }
}

/// Start the process of kernel panicking.
/// Checks whether other cores are panicking and spin early if they do.
/// If no other core has panicked, returns and assumes the caller will eventually call [`kernel_panic_unchecked`].
#[unsafe(no_mangle)]
pub extern "C" fn claim_panic() {
    Arch::disable_irq();
    if IS_PANICKING.fetch_add(1, Ordering::Relaxed) != 0 {
        panic_spin();
    }
}

/// Yes, it's misspelled. Yes, that's intentional.
fn backrtace(mut frame_ptr: *const ()) {
    write_unlocked("**** BEGIN BACKRTACE ****\n");
    const MIN: isize = Arch::FP_LINK_OFFSET.min(Arch::FP_RA_OFFSET);
    const MAX: isize =
        Arch::FP_LINK_OFFSET.max(Arch::FP_RA_OFFSET) + size_of::<usize>() as isize - 1;
    let res: AccessResult<()> = try {
        let mut i = 0;
        loop {
            if i >= 64 {
                printf_unlocked!("<backrtace limited>\n");
                break;
            } else if frame_ptr.is_null() {
                break;
            } else if frame_ptr as isize > 0 {
                printf_unlocked!("<lower-half address>\n");
                break;
            }

            // The frame pointer is *probably* valid.
            let ra_ptr = frame_ptr.wrapping_byte_add(Arch::FP_RA_OFFSET as usize);
            let ra = Arch::fallible_load_usize(ra_ptr as _)?;
            printf_unlocked!("0x{:x}\n", ra - 1);

            let link_ptr = frame_ptr.wrapping_byte_add(Arch::FP_LINK_OFFSET as usize);
            frame_ptr = Arch::fallible_load_usize(link_ptr as _)? as _;

            i += 1;
        }
    };
    if let Err(x) = res {
        printf_unlocked!("<{}>\n", x);
    }
    write_unlocked("**** END BACKRTACE ****\n");
}

fn kekw() {
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

fn panic_spin() -> ! {
    Arch::disable_irq();
    loop {
        Arch::pause_hint();
    }
}
