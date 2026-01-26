// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

// This subsystem is implemented in Rust.

#pragma once

#include "badgeros-abi/errno.h"
#include "time.h"

#include <stddef.h>



// Seek mode for `file_seek`.
typedef enum {
    SEEK_SET,
    SEEK_CUR,
    SEEK_END,
} seek_mode_t;

// Types of a file node.
typedef enum {
    // Unknown file type (not valid for make_file_spec_t).
    NODE_TYPE_UNKNOWN,
    // Named pipe.
    NODE_TYPE_FIFO,
    // Character device.
    NODE_TYPE_CHAR_DEV,
    // Directory.
    NODE_TYPE_DIRECTORY,
    // Block device.
    NODE_TYPE_BLOCK_DEV,
    // Regular file.
    NODE_TYPE_REGULAR,
    // Symbolic link.
    NODE_TYPE_SYMLINK,
    // UNIX domain socket.
    NODE_TYPE_UNIX_SOCKET,
} node_type_t;

// Opaque file handle; represents an `Arc<dyn File>`.
typedef struct {
    void       *data;
    void const *metadata;
} file_t;

// A result of either an errno or a file_t.
typedef struct {
    errno_t errno;
    file_t  file;
} errno_file_t;

// Specifies how a file is to be created.
typedef struct {
    // The type of file to create.
    node_type_t type;
    union {
        // ID of the character device to attach.
        uint32_t char_dev;
        struct {
            // Block device ID.
            uint32_t block_dev;
            // Has partition offset and size.
            bool     is_partition;
            // Partition offset and size.
            uint64_t part_offset, part_size;
        };
        // The symlink target.
        struct {
            // The path to the target of the symlink.
            char const *target;
            // The length of the target path.
            size_t      target_len;
        } symlink;
    };
} make_file_spec_t;

// Filesystem media types.
typedef enum {
    // Block device as media.
    FS_MEDIA_BLKDEV,
    // Linear area of RAM as media.
    FS_MEDIA_RAM,
    // Loopback file as media,
    // FS_MEDIA_FILE,
} fs_media_type_t;

// Filesystem media descriptor.
typedef struct {
    // Media type.
    fs_media_type_t type;
    // Partition offset added automatically.
    uint64_t        part_offset;
    // Partition length.
    uint64_t        part_length;
    union {
        // Block device ID.
        uint32_t blkdev;
        // RAM area.
        uint8_t *ram;
        // TODO: Loopback file support.
    };
} fs_media_t;

// Inode statistics from `file_stat`.
typedef struct {
    // ID and class of device containing file.
    uint64_t   dev;
    // Inode number.
    uint64_t   ino;
    // File type and mode flags
    uint16_t   mode;
    // Number of hard links.
    uint16_t   nlink;
    // Owner user ID.
    uint16_t   uid;
    // Owner group ID.
    uint16_t   gid;
    // ID of device for device special files.
    uint64_t   rdev;
    // Byte size of this file.
    uint64_t   size;
    // Block size for filesystem I/O.
    uint64_t   blksize;
    // Number of 512 byte blocks allocated (represents actual used disk space).
    uint64_t   blocks;
    // Time of last access. On BadgerOS, only updated when modified or created.
    timespec_t atim;
    // Time of last modification.
    timespec_t mtim;
    // Time of last status change.
    timespec_t ctim;
} stat_t;

// Return value of `fs_pipe`.
typedef struct {
    errno_t errno;
    file_t  write_end, read_end;
} fs_pipe_t;



// Returns the errno on error or passes through the value on success.
#define RETURN_ON_ERRNO_FILE(expr, ...)                                                                                \
    ({                                                                                                                 \
        errno_file_t tmp = (expr);                                                                                     \
        if (tmp.errno < 0) {                                                                                           \
            __VA_ARGS__;                                                                                               \
            return tmp.errno;                                                                                          \
        }                                                                                                              \
        tmp.file;                                                                                                      \
    })

// Represents absence of a file.
#define FILE_NONE ((file_t){NULL, NULL})

// Allows for reading the file.
#define FS_O_READ_ONLY  0x00000001
// Allows for writing the file.
#define FS_O_WRITE_ONLY 0x00000002
// Allows for both reading and writing.
#define FS_O_READ_WRITE 0x00000003
// Makes writing work in append mode.
#define FS_O_APPEND     0x00000004
// Fail if the target is a directory.
#define FS_O_FILE_ONLY  0x00000008
// Fail if the target is not a directory.
#define FS_O_DIR_ONLY   0x00000010
// Do not follow the last symlink.
#define FS_O_NOFOLLOW   0x00000020
// Create the file if it does not exist.
#define FS_O_CREATE     0x00000040
// Fail if the file exists already.
#define FS_O_EXCLUSIVE  0x00000080
// Truncate the file on open.
#define FS_O_TRUNCATE   0x00000100

// Filesystem is read-only.
#define FS_M_READ_ONLY 0x00000001
/// Do not follow symbolic links.
#define FS_M_NOFOLLOW  0x00000020
/// Try to cancel pending I/O operations; only supported on certain filesystems.
#define FS_M_FORCE     0x00010000
/// Lazily unmount; remove the filesystem from the tree now, and wait with unmount until open handles are closed.
#define FS_M_DETACH    0x00020000

// The maximum number of symlinks followed.
#define LINK_MAX 32
// The maximum path length.
#define PATH_MAX 4096
// The maximum filename length.
#define NAME_MAX 255



// Mount a filesystem.
errno_t
    fs_mount(file_t at, char const *path, size_t path_len, char const *type, fs_media_t *media, uint32_t mountflags);
// Unmount a filesystem given mountpoint or media.
errno_t fs_umount(file_t at, char const *path, size_t path_len, uint32_t mountflags);
// Auto-mount the root filesystem according to kernel parameters, or panic if impossible.
void    fs_mount_root_fs();

// Create an unnamed pipe.
fs_pipe_t fs_pipe(uint32_t oflags);

// Open a file.
errno_file_t fs_open(file_t at, char const *path, size_t path_len, uint32_t oflags);
// Drop a share from a file descriptor.
void         fs_file_drop(file_t file);
// Create another share for a file descriptor.
file_t       fs_file_clone(file_t file);

// Create a new name for a tile.
errno_t fs_link(
    file_t      old_at,
    char const *old_path,
    size_t      old_path_len,
    file_t      new_at,
    char const *new_path,
    size_t      new_path_len,
    uint32_t    append
);
// Remove a file or directory.
// Uses POSIX `rmdir` semantics iff `is_rmdir`, otherwise POSIX unlink semantics.
errno_t fs_unlink(file_t at, char const *path, size_t path_len, int is_rmdir);
// Create a new file or directory.
// The spec parameter should be defined as needed for NewFileSpec.
errno_t fs_make_file(file_t at, char const *path, size_t path_len, make_file_spec_t spec);
// Rename a file within the same filesystem.
errno_t fs_rename(
    file_t      old_at,
    char const *old_path,
    size_t      old_path_len,
    file_t      new_at,
    char const *new_path,
    size_t      new_path_len,
    uint32_t    flags
);
// Get the real path from some canonical path.
// The result is written to out_path, which must have at least out_path_len bytes.
errno_t fs_realpath(
    file_t at, char const *path, size_t path_len, int follow_last_symlink, char **out_path, size_t *out_path_len
);

// Get the device that this file represents, if any.
bool         fs_get_device(file_t file, uint32_t *id_out);
// Get the partition offset and size that this file represents, if any.
bool         fs_get_part_offset(file_t file, uint64_t *offset_out, uint64_t *size_out);
// Get the stat info for this file's inode.
errno_t      fs_stat(file_t file, stat_t *stat_out);
// Get the position in the file.
errno64_t    fs_tell(file_t file);
// Change the position in the file.
errno64_t    fs_seek(file_t file, seek_mode_t mode, int64_t offset);
// Write bytes to this file.
errno_size_t fs_write(file_t file, void const *wdata, size_t wdata_len);
// Read bytes from this file.
errno_size_t fs_read(file_t file, void *rdata, size_t rdata_len);
// Resize the file to a new length.
errno_t      fs_resize(file_t file, uint64_t new_size);
// Sync the underlying caches to disk.
errno_t      fs_sync(file_t file);
