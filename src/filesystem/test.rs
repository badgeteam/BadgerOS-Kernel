// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::vec::Vec;

use crate::{
    filesystem::{oflags, open},
    ktest_expect, rootfs_ktest,
};

rootfs_ktest! { FILE_READ_BLOCK,
    let fd = open(None, b"/testdata0.bin", oflags::READ_ONLY)?;

    let stat = fd.stat()?;
    ktest_expect!(stat.size, 32*1024);

    let mut buf = Vec::try_with_capacity(32*1024)?;
    buf.resize(32*1024, 0);
    ktest_expect!(fd.readk(&mut buf)?, 32*1024);

    for i in 0..32 {
        for x in 0..1024 {
            ktest_expect!(buf[i*1024 + x], i as u8, [i, x]);
        }
    }
}

rootfs_ktest! { FILE_READ_SUBBLOCK,
    let fd = open(None, b"/testdata1.bin", oflags::READ_ONLY)?;

    let stat = fd.stat()?;
    ktest_expect!(stat.size, 32*1024);

    let mut buf = Vec::try_with_capacity(32*1024)?;
    buf.resize(32*1024, 0);
    ktest_expect!(fd.readk(&mut buf)?, 32*1024);

    for i in 0..32 {
        for x in 0..1024 {
            ktest_expect!(buf[i*1024 + x], (x / 4) as u8, [i, x]);
        }
    }
}
