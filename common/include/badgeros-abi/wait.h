#ifndef _ABIBITS_WAIT_H
#define _ABIBITS_WAIT_H

#define WNOHANG    1
#define WUNTRACED  2
#define WSTOPPED   2
#define WEXITED    4
#define WCONTINUED 8
#define WNOWAIT    0x01000000

#define WCOREFLAG 0x80

#define WEXITSTATUS(x) (((status) & 0xff00) >> 8)
#define WTERMSIG(x)    (((status) & 0xff00) >> 8)
#define WSTOPSIG(x)    (((status) & 0xff00) >> 8)

// Whether the child exited normally (by means of `SYSCALL_PROC_EXIT`).
#define WIFEXITED(status)    (((status) & 0xff) == 0)
// Whether the child was killed by a signal.
#define WIFSIGNALED(status)  ((status) & 0x40)
// Whether the child was suspended by a signal.
#define WIFSTOPPED(status)   ((status) & 0x20)
// Whether the child was resumed by `SIGCONT`.
#define WIFCONTINUED(status) ((status) & 0x10)
// Whether the child dumped core.
#define WCOREDUMP(x)         ((x) & __WCOREFLAG)

#endif
