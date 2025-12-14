
// SPDX-License-Identifier: MIT

#pragma once

#define AT_FDCWD -100

// Allows for reading the file.
#define O_RDONLY    0x00000001
// Allows for writing the file.
#define O_WRONLY    0x00000002
// Allows for both reading and writing.
#define O_RDWR      0x00000003
// Makes writing work in append mode.
#define O_APPEND    0x00000004
// Fail if the target is a directory.
#define O_FILE_ONLY 0x00000008
// Fail if the target is not a directory.
#define O_DIRECTORY 0x00000010
// Do not follow the last symlink.
#define O_NOFOLLOW  0x00000020
// Create the file if it does not exist.
#define O_CREAT     0x00000040
// Fail if the file exists already.
#define O_EXCL      0x00000080
// Truncate the file on open.
#define O_TRUNC     0x00000100
// Use non-blocking I/O.
#define O_NONBLOCK  0x00000200
// Close file on exec.
#define O_CLOEXEC   0x00010000
