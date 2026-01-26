
// SPDX-License-Identifier: MIT

#pragma once

#include "badgeros-abi/pid_t.h"
#include "badgeros-abi/sigset_t.h"
#include "badgeros-abi/sigval.h"
#include "badgeros-abi/uid_t.h"

#include <stddef.h>
#include <stdint.h>

// NOLINTBEGIN

// struct taken from musl.
typedef struct {
    int si_signo, si_errno, si_code;
    union {
        char __pad[128 - 2 * sizeof(int) - sizeof(long)];
        struct {
            union {
                struct {
                    pid_t si_pid;
                    uid_t si_uid;
                } __piduid;
                struct {
                    int si_timerid;
                    int si_overrun;
                } __timer;
            } __first;
            union {
                union sigval si_value;
                struct {
                    int  si_status;
                    long si_utime, si_stime;
                } __sigchld;
            } __second;
        } __si_common;
        struct {
            void *si_addr;
            short si_addr_lsb;
            union {
                struct {
                    void *si_lower;
                    void *si_upper;
                } __addr_bnd;
                unsigned si_pkey;
            } __first;
        } __sigfault;
        struct {
            long si_band;
            int  si_fd;
        } __sigpoll;
        struct {
            void    *si_call_addr;
            int      si_syscall;
            unsigned si_arch;
        } __sigsys;
    } __si_fields;
} siginfo_t;


typedef struct __stack {
    void  *ss_sp;
    int    ss_flags;
    size_t ss_size;
} stack_t;


#ifdef __riscv

#define NGREG 32

enum {
    REG_PC = 0,
#define REG_PC REG_PC
    REG_RA = 1,
#define REG_RA REG_RA
    REG_SP = 2,
#define REG_SP REG_SP
    REG_TP = 4,
#define REG_TP REG_TP
    REG_S0 = 8,
#define REG_S0 REG_S0
    REG_A0 = 10
#define REG_A0 REG_A0
};

struct __riscv_f_ext_state {
    uint32_t f[32];
    uint32_t fcsr;
};

struct __riscv_d_ext_state {
    uint64_t f[32];
    uint32_t fcsr;
};

struct __riscv_q_ext_state {
    uint64_t f[64] __attribute__((__aligned__(16)));
    uint32_t fcsr;
    uint32_t reserved[3];
};

union __riscv_fp_state {
    struct __riscv_f_ext_state f;
    struct __riscv_d_ext_state d;
    struct __riscv_q_ext_state q;
};

typedef unsigned long __riscv_mc_gp_state[NGREG];

typedef struct sigcontext {
    __riscv_mc_gp_state    gregs;
    union __riscv_fp_state fpregs;
} mcontext_t;

typedef struct __ucontext {
    unsigned long      uc_flags;
    struct __ucontext *uc_link;
    stack_t            uc_stack;
    sigset_t           uc_sigmask;
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wpedantic"
    uint8_t __unused[1024 / 8 - sizeof(sigset_t)];
#pragma GCC diagnostic pop
    mcontext_t uc_mcontext;
} ucontext_t;

#endif

// NOLINTEND
