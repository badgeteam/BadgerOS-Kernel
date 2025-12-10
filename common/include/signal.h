
// SPDX-License-Identifier: MIT

#pragma once

#include <stddef.h>
#include <stdint.h>

#include <sys/types.h>

#define SIGHUP    1
#define SIGINT    2
#define SIGQUIT   3
#define SIGILL    4
#define SIGTRAP   5
#define SIGABRT   6
#define SIGBUS    7
#define SIGFPE    8
#define SIGKILL   9
#define SIGUSR1   10
#define SIGSEGV   11
#define SIGUSR2   12
#define SIGPIPE   13
#define SIGALRM   14
#define SIGTERM   15
#define SIGSTKFLT 16
#define SIGCHLD   17
#define SIGCONT   18
#define SIGSTOP   19
#define SIGTSTP   20
#define SIGTTIN   21
#define SIGTTOU   22
#define SIGURG    23
#define SIGXCPU   24
#define SIGXFSZ   25
#define SIGVTALRM 26
#define SIGPROF   27
#define SIGWINCH  28
#define SIGIO     29
#define SIGPWR    30
#define SIGSYS    31

#define SIG_COUNT 32



// Information associated with a received signal.
typedef struct {
    // Signal number.
    int   si_signo;
    // Signal code.
    int   si_code;
    // Sending process ID.
    pid_t si_pid;
    // Real user ID of sending process.
    uid_t si_uid;
    // Memory location which caused the signal.
    void *si_addr;
    // Exit value or signal.
    int   si_status;
} siginfo_t;

// Set of signals.
typedef uint32_t sigset_t;

// Describes the actions to take for a signal.
struct sigaction {
    union {
        // Old-style signal handler.
        void (*sa_handler)(int signo);
        // New-style signal handler.
        void (*sa_sigaction)(int signo, siginfo_t *info, void *ucontext);
        void *sa_handler_ptr;
    };
    // Signal mask to temporarily apply.
    sigset_t sa_mask;
    int      sa_flags;
    void    *sa_return_trampoline;
};

// User context saved on the stack by the kernel.
typedef struct {
    siginfo_t siginfo;
    sigset_t  prev_sigmask;
    struct {
#ifdef __riscv
        size_t t0, t1;
        size_t t2, a0, a1, a2, a3, a4, a5, a6, a7;
        size_t t3, t4, t5, t6;
        size_t pc;
        size_t s0;
        size_t ra;
#endif
#ifdef __x86_64__
#endif
    } regs;
} ucontext_t;

#ifdef BADGEROS_KERNEL

// Ignore this signal.
#define SIG_IGN ((size_t)0)
// Assign the default action to this signal.
#define SIG_DFL SIZE_MAX
// Bitmask of signals that kill the process by default.
#define SIG_DFL_KILL_MASK                                                                                              \
    ((1llu << SIGHUP) | (1llu << SIGSEGV) | (1llu << SIGILL) | (1llu << SIGTRAP) | (1llu << SIGTERM) |                 \
     (1llu << SIGKILL) | (1llu << SIGABRT) | (1llu << SIGSYS))
// Signal name table.
extern char const *signames[SIG_COUNT];

#else

// Ignore this signal.
#define SIG_IGN ((void *)0)
// Assign the default action to this signal.
#define SIG_DFL ((void *)-1)

// Signal handler.
typedef void (*sighandler_t)(int signum);

// Set the handler or action for a particular signal.
sighandler_t signal(int signum, sighandler_t handler);

#endif
