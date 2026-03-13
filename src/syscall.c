
// SPDX-License-Identifier: MIT

#include "syscall.h"



// Table of system calls.
static syscall_info_t systab[] = {
#define SYSCALL_CAST (void (*)())(void *)
#define SYSCALL_DEF(no, enum, name, returns, ...)                                                                      \
    [no] = {                                                                                                           \
        SYSCALL_CAST name,                                                                                             \
        sizeof(returns),                                                                                               \
        SYSCALL_FLAG_EXISTS,                                                                                           \
    },
#define SYSCALL_DEF_V(no, enum, name, ...)                                                                             \
    [no] = {                                                                                                           \
        SYSCALL_CAST name,                                                                                             \
        0,                                                                                                             \
        SYSCALL_FLAG_EXISTS,                                                                                           \
    },
#define SYSCALL_DEF_F(no, enum, name, returns, ...)                                                                    \
    [no] = {                                                                                                           \
        SYSCALL_CAST name,                                                                                             \
        sizeof(returns),                                                                                               \
        SYSCALL_FLAG_EXISTS | SYSCALL_FLAG_FAST,                                                                       \
    },
#define SYSCALL_DEF_FV(no, enum, name, ...)                                                                            \
    [no] = {                                                                                                           \
        SYSCALL_CAST name,                                                                                             \
        0,                                                                                                             \
        SYSCALL_FLAG_EXISTS | SYSCALL_FLAG_FAST,                                                                       \
    },
#include "syscall_defs.inc"
};
static int const systab_len = sizeof(systab) / sizeof(syscall_info_t);

syscall_info_t syscall_info(int no) {
    if (no < 0 || no >= systab_len) {
        return (syscall_info_t){NULL, 0, 0};
    } else {
        return systab[no];
    }
}
