
// SPDX-License-Identifier: MIT

#pragma once

#ifdef __ASSEMBLER__

// Assembly enum.
#define SYSCALL_DEF(num, enum, name, returns, ...) .equ enum, num
#include "syscall_defs.inc"

#else

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef BADGEROS_KERNEL
#include "filesystem.h"
#include "scheduler/scheduler.h"

#define SYSCALL_FLAG_EXISTS 0x01
#define SYSCALL_FLAG_FAST   0x02

typedef struct {
    void (*funcptr)();
    uint8_t retwidth;
    uint8_t flags;
} syscall_info_t;

syscall_info_t syscall_info(int no);

#else
struct sigaction;
typedef int         tid_t;
typedef struct stat stat_t;

// Process is running or waiting for syscalls.
#define PROC_RUNNING     0x00000001
// Process is waiting for threads to exit.
#define PROC_EXITING     0x00000002
// Process has fully exited.
#define PROC_EXITED      0x00000004

// Open for read-only.
#define OFLAGS_READONLY  0x00000001
// Open for write-only.
#define OFLAGS_WRITEONLY 0x00000002
// Open for read and write.
#define OFLAGS_READWRITE 0x00000003
// Seek to the end on opening.
#define OFLAGS_APPEND    0x00000004
// Truncate on opening.
#define OFLAGS_TRUNCATE  0x00000008
// Create if it doesn't exist on opening.
#define OFLAGS_CREATE    0x00000010
// Error if it exists on opening.
#define OFLAGS_EXCLUSIVE 0x00000020
// Do not inherit to child process.
#define OFLAGS_CLOEXEC   0x00000040
// Open a directory instead of a file.
#define OFLAGS_DIRECTORY 0x00000080
// Do not block on sockets, FIFOs, TTYs, etc.
#define OFLAGS_NONBLOCK  0x00000100

#define MEMFLAGS_R   0x00000001
#define MEMFLAGS_W   0x00000002
#define MEMFLAGS_RW  0x00000003
#define MEMFLAGS_X   0x00000004
#define MEMFLAGS_RX  0x00000005
#define MEMFLAGS_WX  0x00000006
#define MEMFLAGS_RWX 0x00000007
#endif

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// C enum.
typedef enum {
#define SYSCALL_DEF(num, enum, name, returns, ...) enum = num,
#include "syscall_defs.inc"
} syscall_t;

// C declarations.
#define SYSCALL_DEF(num, enum, name, returns, ...) returns name(__VA_ARGS__);
#include "syscall_defs.inc"

#endif
