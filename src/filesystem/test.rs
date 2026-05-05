// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::vec::Vec;

use crate::{
    bindings::error::Errno,
    config::PAGE_SIZE,
    cpu::usercopy::{fallible_load_u8, fallible_store_u8},
    filesystem::{oflags, open, unlink},
    ktest_assert, ktest_expect,
    mem::vmm::{
        kernel_mm,
        map::{self, Mapping},
        prot, zeroes,
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

rootfs_ktest! { FILE_MAP_DENYWRITE,
    // Make a file and make it two pages long.
    let fd = open(None, b"/denywrite.bin", oflags::CREATE | oflags::READ_WRITE | oflags::TRUNCATE)?;
    ktest_expect!(fd.writek(zeroes())?, PAGE_SIZE as usize);
    ktest_expect!(fd.writek(zeroes())?, PAGE_SIZE as usize);
    let stat = fd.stat()?;
    ktest_expect!(stat.size, 2 * PAGE_SIZE as u64);

    let memobject = fd.get_memobject().ok_or(Errno::EACCES)?;

    unsafe {
        // A mapping with DENYWRITE should fail while the FD is open.
        ktest_expect!(kernel_mm().map(
            stat.size as usize,
            0,
            map::SHARED | map::LAZY_KERNEL | map::DENYWRITE,
            prot::READ | prot::WRITE,
            Some(
                Mapping {
                    offset: 0,
                    object: memobject.clone(),
                }
            )
        ), Err(Errno::ETXTBSY));

        // After closing the FD, it should succeed.
        drop(fd);
        let vaddr = kernel_mm().map(
            stat.size as usize,
            0,
            map::SHARED | map::LAZY_KERNEL | map::DENYWRITE,
            prot::READ | prot::WRITE,
            Some(
                Mapping {
                    offset: 0,
                    object: memobject.clone()
                }
            )
        )?;

        // The FD should now not open for writing.
        ktest_expect!(open(None, b"/denywrite.bin", oflags::READ_WRITE).err(), Some(Errno::ETXTBSY));
        let fd = open(None, b"/denywrite.bin", oflags::READ_ONLY)?;
        ktest_expect!(fd.set_flags(oflags::READ_WRITE).err(), Some(Errno::ETXTBSY));

        // After unmapping, opening should succeed.
        kernel_mm().unmap(vaddr..vaddr + stat.size as usize)?;
        fd.set_flags(oflags::READ_WRITE)?;
        drop(open(None, b"/denywrite.bin", oflags::READ_WRITE)?);

        // It should not be mappable again.
        ktest_expect!(kernel_mm().map(
            stat.size as usize,
            0,
            map::SHARED | map::LAZY_KERNEL | map::DENYWRITE,
            prot::READ | prot::WRITE,
            Some(
                Mapping {
                    offset: 0,
                    object: memobject.clone(),
                }
            )
        ), Err(Errno::ETXTBSY));

        // Clearing the write access flag one last time.
        fd.set_flags(oflags::READ_ONLY)?;

        // Mapping should succeed once more.
        let vaddr = kernel_mm().map(
            stat.size as usize,
            0,
            map::SHARED | map::LAZY_KERNEL | map::DENYWRITE,
            prot::READ | prot::WRITE,
            Some(
                Mapping {
                    offset: 0,
                    object: memobject.clone()
                }
            )
        )?;
        kernel_mm().unmap(vaddr..vaddr + stat.size as usize)?;
    }

    // Delete the file now that it's unneeded.
    unlink(None, b"/denywrite.bin", false)?;
}

rootfs_ktest! { FILE_MAP_RESIZE,
    // Make a file and make it two pages long.
    let fd = open(None, b"/resizetest.bin", oflags::CREATE | oflags::READ_WRITE | oflags::TRUNCATE)?;
    ktest_expect!(fd.writek(zeroes())?, PAGE_SIZE as usize);
    ktest_expect!(fd.writek(zeroes())?, PAGE_SIZE as usize);
    let stat = fd.stat()?;
    ktest_expect!(stat.size, 2 * PAGE_SIZE as u64);

    unsafe {
        // Memory-map the file and check that we can access both pages.
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

        // Before resize: both pages accessible.
        fallible_store_u8(ptr, 1)?;
        fallible_store_u8(ptr.add(PAGE_SIZE as usize), 2)?;

        // After resize: second page no longer accessible.
        fd.resize(PAGE_SIZE as u64)?;
        fallible_store_u8(ptr, 1)?;
        ktest_assert!(fallible_store_u8(ptr.add(PAGE_SIZE as usize), 2).is_err());

        // Fractional page size: first page still accessible, OOB data zeroed.
        fallible_store_u8(ptr.add(42), 9)?;
        fd.resize(42)?;
        ktest_expect!(fallible_load_u8(ptr.add(42))?, 0);

        // Complete truncation: no access at all.
        fd.resize(0)?;
        ktest_assert!(fallible_store_u8(ptr, 1).is_err());
        ktest_assert!(fallible_store_u8(ptr.add(PAGE_SIZE as usize), 2).is_err());

        // Make it bigger again: data still zeroes but pages accessible again.
        fd.resize(2*PAGE_SIZE as u64)?;
        ktest_expect!(fallible_load_u8(ptr)?, 0);
        ktest_expect!(fallible_load_u8(ptr.add(42))?, 0);
        ktest_expect!(fallible_load_u8(ptr.add(PAGE_SIZE as usize))?, 0);

        // Clean up by unmapping.
        kernel_mm().unmap(vaddr..vaddr + stat.size as usize)?;
    }

    // Delete the file now that it's unneeded.
    unlink(None, b"/resizetest.bin", false)?;
}
