
// SPDX-License-Identifier: MIT

#pragma once

// Don't block waiting.
#define WNOHANG    0x00000001
// Match children that have stopped but not been traced.
#define WUNTRACED  0x00000002
// Report continued child.
#define WCONTINUED 0x00000008
// Don't reap, just poll status.
#define WNOWAIT    0x01000000

// Create status for child exited normally.
#define W_EXITED(ret)    ((ret) << 8)
// Create status for child exited by signal.
#define W_SIGNALLED(sig) (((sig) << 8) | 0x40)
// Create status for child that was suspended by a signal.
#define W_STOPCODE(sig)  (((sig) << 8) | 0x20)
// Child was continued.
#define W_CONTINUED      0x10
// Child dumped core.
#define WCOREFLAG        0x80

// What the exit code was.
#define WEXITSTATUS(status) (((status) & 0xff00) >> 8)
// What the terminating signal was.
#define WTERMSIG(status)    WEXITSTATUS(status)
// What signal suspended the child.
#define WSTOPSIG(status)    WEXITSTATUS(status)

// Whether the child dumped core.
#define WCOREDUMP(status)    ((status) & WCOREFLAG)
// Whether the child exited normally (by means of `SYSCALL_PROC_EXIT`).
#define WIFEXITED(status)    (((status) & 0xff) == 0)
// Whether the child was killed by a signal.
#define WIFSIGNALED(status)  ((status) & 0x40)
// Whether the child was suspended by a signal.
#define WIFSTOPPED(status)   ((status) & 0x20)
// Whether the child was resumed by `SIGCONT`.
#define WIFCONTINUED(status) ((status) & 0x10)

// Match any process for `waitpid`.
#define WAIT_ANY    (-1)
// Match processes in current process group for `waitpid`.
#define WAIT_MYPGRP 0

#ifndef BADGEROS_KERNEL
// Wait for a child with a matching PID to die:
// - If `pid` is -1, match any child.
// - If `pid` is 0, match any child in the process group.
// - If `pid` < -1, match any child in the process group `-pid`.
// - If `pid` > 0, match the child with the specified PID.
//
// With certain conditions:
// - If `options & WNOHANG`, returns 0 for children that aren't dead.
// - If `options & WUNTRACED`, also match children that have stopped but not been traced.
// - If a child is traced and stopped, it is always matched.
// - If `options & WCONTINUED`, also match children that were resumed by `SIGCONT`.
//
// Stores the child's status in `wstatus` and returns the PID, or -1 on error.
int waitpid(int pid, int *wstatus, int options);

// Wait for any child to die, store the status in `wstatus` and return the PID.
// Alias for `waitpid(-1, wstatus, 0)`.
static inline int wait(int *wstatus) {
    return waitpid(-1, wstatus, 0);
}
#endif
