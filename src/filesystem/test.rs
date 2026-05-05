// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::vec::Vec;

use crate::{
    bindings::error::Errno,
    filesystem::{oflags, open},
    ktest_expect,
    mem::vmm::{
        kernel_mm,
        map::{self, Mapping},
        prot,
    },
    rootfs_ktest,
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

rootfs_ktest! { FILE_MAP_BLOCK,
    let fd = open(None, b"/testdata0.bin", oflags::READ_ONLY)?;

    let stat = fd.stat()?;
    ktest_expect!(stat.size, 32*1024);

    unsafe {
        let vaddr = kernel_mm().map(
            stat.size as usize,
            0,
            map::SHARED | map::LAZY_KERNEL,
            prot::READ | prot::WRITE,
            Some(
                Mapping {
                    offset: 0,
                    object: fd.get_memobject().ok_or(Errno::EACCES)?
                }
            )
        )?;
        let ptr = vaddr as *mut u8;

        for i in 0..32 {
            for x in 0..1024 {
                ktest_expect!(*ptr.add(i*1024 + x), i as u8, [i, x]);
            }
        }

        kernel_mm().unmap(vaddr..vaddr + stat.size as usize)?;
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

rootfs_ktest! { FILE_WRITE_MARKS_DIRTY,
    let fd = open(None, b"/testdata0.bin", oflags::READ_ONLY)?;

    let stat = fd.stat()?;
    ktest_expect!(stat.size, 32*1024);

    let obj = fd.get_memobject().ok_or(Errno::EACCES)?;
    ktest_expect!(obj.has_dirty_pages(), false);

    unsafe {
        let vaddr = kernel_mm().map(
            stat.size as usize,
            0,
            map::SHARED | map::LAZY_KERNEL,
            prot::READ | prot::WRITE,
            Some(Mapping {
                offset: 0,
                object: obj.clone(),
            })
        )?;
        let ptr = vaddr as *mut u8;

        // Write back the same data — pages are touched but file contents are preserved.
        for i in 0..32usize {
            for x in 0..1024usize {
                core::ptr::write_volatile(ptr.add(i * 1024 + x), i as u8);
            }
        }

        // Dirty is set on unmap when the physmap drops written-back shared pages.
        kernel_mm().unmap(vaddr..vaddr + stat.size as usize)?;
    }

    ktest_expect!(obj.has_dirty_pages(), true);
    fd.sync()?;
    ktest_expect!(obj.has_dirty_pages(), false);
}

rootfs_ktest! { FILE_MAP_SUBBLOCK,
    let fd = open(None, b"/testdata1.bin", oflags::READ_ONLY)?;

    let stat = fd.stat()?;
    ktest_expect!(stat.size, 32*1024);

    unsafe {
        let vaddr = kernel_mm().map(
            stat.size as usize,
            0,
            map::SHARED | map::LAZY_KERNEL,
            prot::READ | prot::WRITE,
            Some(
                Mapping {
                    offset: 0,
                    object: fd.get_memobject().ok_or(Errno::EACCES)?
                }
            )
        )?;
        let ptr = vaddr as *mut u8;

        for i in 0..32 {
            for x in 0..1024 {
                ktest_expect!(*ptr.add(i*1024 + x), (x / 4) as u8, [i, x]);
            }
        }

        kernel_mm().unmap(vaddr..vaddr + stat.size as usize)?;
    }
}
