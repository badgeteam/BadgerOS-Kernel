// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    ffi::{c_char, c_void},
    ptr::{self, null, null_mut, slice_from_raw_parts, slice_from_raw_parts_mut},
};

use alloc::{boxed::Box, sync::Arc};
use static_assertions::assert_eq_size;

use crate::{
    bindings::{
        device::{Device, HasBaseDevice},
        error::Errno,
        raw::{
            self, errno_file_t, errno_size_t, errno_t, errno64_t, file_t, fs_media_t, fs_pipe_t,
            make_file_spec_t, seek_mode_t, stat_t,
        },
    },
    filesystem::{
        Stat,
        media::{Media, MediaType},
    },
    process::usercopy::UserSlice,
};

use super::{File, MakeFileSpec};

/* ==== helper functions ==== */
pub unsafe fn file_into_arc(file: file_t) -> Option<Arc<dyn File>> {
    if file.metadata.is_null() {
        None
    } else {
        Some(unsafe {
            Arc::from_raw(ptr::from_raw_parts_mut(
                file.data,
                core::mem::transmute(file.metadata),
            ))
        })
    }
}

pub unsafe fn file_as_ref(file: file_t) -> Option<&'static dyn File> {
    if file.metadata.is_null() {
        None
    } else {
        Some(unsafe { &*ptr::from_raw_parts(file.data, core::mem::transmute(file.metadata)) })
    }
}

pub fn arc_into_file(arc: Arc<dyn File>) -> file_t {
    let raw = Arc::into_raw(arc);
    file_t {
        data: raw as *mut c_void,
        metadata: unsafe { core::mem::transmute(ptr::metadata(raw)) },
    }
}

pub fn ref_as_file(raw: *const dyn File) -> file_t {
    file_t {
        data: raw as *mut c_void,
        metadata: unsafe { core::mem::transmute(ptr::metadata(raw)) },
    }
}

/* ==== global functions ==== */

#[unsafe(no_mangle)]
unsafe extern "C" fn fs_mount(
    at: file_t,
    path: *const c_char,
    path_len: usize,
    type_: *const c_char,
    media: *const fs_media_t,
    mountflags: u32,
) -> errno_t {
    let at = unsafe { file_as_ref(at) };
    let path = unsafe { &*slice_from_raw_parts(path as *const u8, path_len) };

    let type_ = (!type_.is_null()).then(|| unsafe {
        str::from_utf8_unchecked(&*slice_from_raw_parts(
            type_ as *const u8,
            raw::strlen(type_),
        ))
    });

    Errno::extract(
        try {
            let media = (!media.is_null()).then(|| unsafe { *media });
            let media = if let Some(media) = media {
                Some(match media.type_ {
                    raw::fs_media_type_t_FS_MEDIA_BLKDEV => Media {
                        storage: MediaType::Block(
                            Device::by_id(unsafe { media.__bindgen_anon_1.blkdev })
                                .ok_or(Errno::ENODEV)?
                                .as_block()
                                .ok_or(Errno::ENODEV)?,
                        ),
                        offset: media.part_offset,
                        size: media.part_length,
                    },
                    raw::fs_media_type_t_FS_MEDIA_RAM => Media {
                        storage: MediaType::Ram(UnsafeCell::new(unsafe {
                            Box::from_raw(slice_from_raw_parts_mut(
                                media.__bindgen_anon_1.ram,
                                media.part_length as usize,
                            ))
                        })),
                        offset: media.part_offset,
                        size: media.part_length,
                    },
                    _ => unreachable!(),
                })
            } else {
                None
            };
            super::mount(at, path, type_, media, mountflags)?;
        },
    )
}

#[unsafe(no_mangle)]
unsafe extern "C" fn fs_open(
    at: file_t,
    path: *const c_char,
    path_len: usize,
    oflags: u32,
) -> errno_file_t {
    let at = unsafe { file_as_ref(at) };
    let path = unsafe { &*slice_from_raw_parts(path as *const u8, path_len) };

    match super::open(at, path, oflags) {
        Ok(arc) => errno_file_t {
            errno: 0,
            file: arc_into_file(arc),
        },
        Err(errno) => errno_file_t {
            errno: -(errno as i32),
            file: unsafe { core::mem::zeroed() },
        },
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn fs_file_drop(file: file_t) {
    if !file.metadata.is_null() {
        drop(unsafe { file_into_arc(file) });
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn fs_file_clone(file: file_t) -> file_t {
    debug_assert!(!file.metadata.is_null());
    let arc = unsafe { file_into_arc(file) }.unwrap();
    let clone = arc.clone();
    core::mem::forget(arc);
    arc_into_file(clone)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn fs_link(
    old_at: file_t,
    old_path: *const c_char,
    old_path_len: usize,
    new_at: file_t,
    new_path: *const c_char,
    new_path_len: usize,
    linkflags: u32,
) -> errno_t {
    let old_at = unsafe { file_as_ref(old_at) };
    let old_path = unsafe { &*slice_from_raw_parts(old_path as *const u8, old_path_len) };
    let new_at = unsafe { file_as_ref(new_at) };
    let new_path = unsafe { &*slice_from_raw_parts(new_path as *const u8, new_path_len) };

    Errno::extract(super::link(old_at, old_path, new_at, new_path, linkflags))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn fs_unlink(
    at: file_t,
    path: *const c_char,
    path_len: usize,
    is_rmdir: bool,
) -> errno_t {
    let at = unsafe { file_as_ref(at) };
    let path = unsafe { &*slice_from_raw_parts(path as *const u8, path_len) };

    Errno::extract(super::unlink(at, path, is_rmdir))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn fs_make_file(
    at: file_t,
    path: *const c_char,
    path_len: usize,
    spec: make_file_spec_t,
) -> errno_t {
    let at = unsafe { file_as_ref(at) };
    let path = unsafe { &*slice_from_raw_parts(path as *const u8, path_len) };

    Errno::extract(
        try {
            let spec = match spec.type_ {
                raw::node_type_t_NODE_TYPE_FIFO => MakeFileSpec::Fifo,
                raw::node_type_t_NODE_TYPE_CHAR_DEV => MakeFileSpec::CharDev(
                    Device::by_id(unsafe { spec.__bindgen_anon_1.char_dev })
                        .ok_or(Errno::ENODEV)?
                        .as_char()
                        .ok_or(Errno::ENODEV)?,
                ),
                raw::node_type_t_NODE_TYPE_DIRECTORY => MakeFileSpec::Directory,
                raw::node_type_t_NODE_TYPE_BLOCK_DEV => MakeFileSpec::BlockDev({
                    let block_dev = unsafe { spec.__bindgen_anon_1.__bindgen_anon_1 };
                    (
                        Device::by_id(block_dev.block_dev)
                            .ok_or(Errno::ENODEV)?
                            .as_block()
                            .ok_or(Errno::ENODEV)?,
                        block_dev.is_partition.then_some(
                            block_dev.part_offset..block_dev.part_offset + block_dev.part_size,
                        ),
                    )
                }),
                raw::node_type_t_NODE_TYPE_REGULAR => MakeFileSpec::Regular,
                raw::node_type_t_NODE_TYPE_SYMLINK => MakeFileSpec::Symlink(unsafe {
                    &*slice_from_raw_parts(
                        spec.__bindgen_anon_1.symlink.target as *const u8,
                        spec.__bindgen_anon_1.symlink.target_len,
                    )
                }),
                raw::node_type_t_NODE_TYPE_UNIX_SOCKET => MakeFileSpec::UnixSocket,
                _ => return -(Errno::EINVAL as errno_t),
            };
            super::make_file(at, path, spec)?;
        },
    )
}

#[unsafe(no_mangle)]
unsafe extern "C" fn fs_rename(
    old_at: file_t,
    old_path: *const c_char,
    old_path_len: usize,
    new_at: file_t,
    new_path: *const c_char,
    new_path_len: usize,
    flags: u32,
) -> errno_t {
    let old_at = unsafe { file_as_ref(old_at) };
    let old_path = unsafe { &*slice_from_raw_parts(old_path as *const u8, old_path_len) };
    let new_at = unsafe { file_as_ref(new_at) };
    let new_path = unsafe { &*slice_from_raw_parts(new_path as *const u8, new_path_len) };

    Errno::extract(super::rename(old_at, old_path, new_at, new_path, flags))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn fs_realpath(
    at: file_t,
    path: *const c_char,
    path_len: usize,
    follow_last_symlink: bool,
    out_path: *mut *mut c_char,
    out_path_len: *mut usize,
) -> errno_t {
    let at = unsafe { file_as_ref(at) };
    let path = unsafe { &*slice_from_raw_parts(path as *const u8, path_len) };

    match super::realpath(at, path, follow_last_symlink) {
        Ok(path) => {
            unsafe {
                let parts = path.into_raw_parts();
                *out_path = parts.0 as *mut c_char;
                *out_path_len = parts.1;
            }
            0 // Success
        }
        Err(errno) => -(errno as errno_t),
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn fs_pipe(oflags: u32) -> fs_pipe_t {
    match super::pipe(oflags) {
        Ok((read, write)) => fs_pipe_t {
            errno: 0,
            write_end: arc_into_file(write),
            read_end: arc_into_file(read),
        },
        Err(x) => fs_pipe_t {
            errno: -(x as errno_t),
            write_end: file_t {
                data: null_mut(),
                metadata: null(),
            },
            read_end: file_t {
                data: null_mut(),
                metadata: null(),
            },
        },
    }
}

/* ==== file descriptor functions ==== */

#[unsafe(no_mangle)]
pub extern "C" fn fs_get_device(file: file_t, id_out: *mut u32) -> bool {
    let file = unsafe { file_as_ref(file) }.unwrap();
    match file.get_device() {
        Some(dev) => {
            unsafe { *id_out = dev.id().into() };
            true
        }
        None => false,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fs_get_part_offset(
    file: file_t,
    offset_out: *mut u64,
    size_out: *mut u64,
) -> bool {
    let file = unsafe { file_as_ref(file) }.unwrap();
    let res = file.get_part_offset();
    match &res {
        Some(part) => unsafe {
            *offset_out = part.start;
            *size_out = part.end - part.start;
        },
        None => (),
    }
    res.is_some()
}

#[unsafe(no_mangle)]
pub extern "C" fn fs_stat(file: file_t, stat_out: *mut stat_t) -> errno_t {
    let file = unsafe { file_as_ref(file) }.unwrap();
    assert_eq_size!(stat_t, Stat);
    match file.stat() {
        Ok(stat) => {
            unsafe { *(stat_out as *mut Stat) = stat };
            0
        }
        Err(err) => -(err as errno_t),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fs_tell(file: file_t) -> errno64_t {
    let file = unsafe { file_as_ref(file) }.unwrap();
    Errno::extract_u64(file.tell())
}

#[unsafe(no_mangle)]
pub extern "C" fn fs_seek(file: file_t, mode: seek_mode_t, offset: i64) -> errno64_t {
    let file = unsafe { file_as_ref(file) }.unwrap();
    let mode = unsafe { core::mem::transmute(mode as u32) };
    Errno::extract_u64(file.seek(mode, offset))
}

#[unsafe(no_mangle)]
pub extern "C" fn fs_write(file: file_t, wdata: *const c_void, wdata_len: usize) -> errno_size_t {
    let file = unsafe { file_as_ref(file) }.unwrap();
    let wdata = unsafe { &*slice_from_raw_parts(wdata as *const u8, wdata_len) };
    Errno::extract_usize(file.write(UserSlice::new_kernel(wdata)))
}

#[unsafe(no_mangle)]
pub extern "C" fn fs_read(file: file_t, rdata: *mut c_void, rdata_len: usize) -> errno_size_t {
    let file = unsafe { file_as_ref(file) }.unwrap();
    let rdata = unsafe { &mut *slice_from_raw_parts_mut(rdata as *mut u8, rdata_len) };
    Errno::extract_usize(file.read(UserSlice::new_kernel_mut(rdata)))
}

#[unsafe(no_mangle)]
pub extern "C" fn fs_resize(file: file_t, new_size: u64) -> errno_t {
    let file = unsafe { file_as_ref(file) }.unwrap();
    Errno::extract(file.resize(new_size))
}

#[unsafe(no_mangle)]
pub extern "C" fn fs_sync(file: file_t) -> errno_t {
    let file = unsafe { file_as_ref(file) }.unwrap();
    Errno::extract(file.sync())
}
