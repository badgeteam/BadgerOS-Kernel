
// SPDX-License-Identifier: MIT
// clang-format off



/* ==== DEFAULT MACRO EXPANSIONS ==== */

// Normal syscall definition.
#ifndef SYSCALL_DEF
#define SYSCALL_DEF(no, enum, name, returns, ...)
#endif

// Normal syscall definition (returning void).
#ifndef SYSCALL_DEF_V
#define SYSCALL_DEF_V(no, enum, name, ...) SYSCALL_DEF(no, enum, name, void, __VA_ARGS__)
#endif




/* ==== THREAD SYSCALLS ==== */
// Implemented in process/syscall_impl.c

// Yield to other threads.
SYSCALL_DEF_V(1, SYSCALL_THREAD_YIELD, syscall_thread_yield)

// Sleep for a certain amount of time.
SYSCALL_DEF(53, SYSCALL_THREAD_SLEEP, syscall_thread_sleep, int, int64_t delay)

// Create a new thread.
// Returns thread ID or -errno.
SYSCALL_DEF(2, SYSCALL_THREAD_CREATE, syscall_thread_create, long, void *entry, void *arg, int priority)

// Detach a thread; the thread will be destroyed as soon as it exits.
// Returns 0 or -errno.
SYSCALL_DEF(3, SYSCALL_THREAD_DETACH, syscall_thread_detach, int, long thread)

// Wait for a thread to stop and return its exit code.
// Returns the exit code of that thread or -errno.
SYSCALL_DEF(4, SYSCALL_THREAD_JOIN, syscall_thread_join, int, long thread)

// Exit the current thread; exit code can be read unless destroyed or detached.
SYSCALL_DEF_V(5, SYSCALL_THREAD_EXIT, syscall_thread_exit, int code)



/* ==== PROCESS MANAGEMENT SYSCALLS ==== */
// Implemented in process/syscall_impl.c

// Exit the process; exit code can be read by parent process.
SYSCALL_DEF_V(8, SYSCALL_PROC_EXIT, syscall_proc_exit, int code)

// Create a copy of the running process and return its PID (to the parent) or -1 (to the child).
SYSCALL_DEF(10, SYSCALL_PROC_FORK, syscall_proc_fork, int64_t)

// Execute the program at `path`, replacing the calling program's code and data in the process.
SYSCALL_DEF(11, SYSCALL_PROC_EXEC, syscall_proc_exec, int, char const *path, char const *const *argv, char const *const *envp)

// Reserved for POSIX spawn.
// SYSCALL_DEF(12, SYSCALL_PROC_SPAWN, int, ...)

// Set the signal handler for a specific signal number.
SYSCALL_DEF(13, SYSCALL_PROC_SIGACTION, syscall_proc_sigaction, int, int signum, struct sigaction const *newhandler, struct sigaction *oldhandler)

// Return from a signal handler.
SYSCALL_DEF_V(14, SYSCALL_PROC_SIGRET, syscall_proc_sigret)

// Get child process status update.
SYSCALL_DEF(15, SYSCALL_PROC_WAITPID, syscall_proc_waitpid, int, int pid, int *wstatus, int options)


/* ==== FILESYSTEM SYSCALLS ==== */
// Implemented in filesystem/syscall_impl.c

// Open a file, optionally relative to a directory.
// If `at` is -1, `path` is relative to the working directory.
// Returns -errno on error, file descriptor number on success.
SYSCALL_DEF(16, SYSCALL_FS_OPEN, syscall_fs_open, int, int at, char const *path, int oflags)

// Flush and close a file.
SYSCALL_DEF(17, SYSCALL_FS_CLOSE, syscall_fs_close, int, int fd)

// Read bytes from a file.
// Returns 0 on EOF, -errno on error, read count on success.
SYSCALL_DEF(18, SYSCALL_FS_READ, syscall_fs_read, int64_t, int fd, void *read_buf, long read_len)

// Write bytes to a file.
// Returns -errno on error, write count on success.
SYSCALL_DEF(19, SYSCALL_FS_WRITE, syscall_fs_write, int64_t, int fd, void const *write_buf, long write_len)

// Read directory entries from a directory handle.
// See `dirent_t` for the format.
// Returns -errno on error, read count on success.
SYSCALL_DEF(20, SYSCALL_FS_GETDENTS, syscall_fs_getdents, int64_t, int fd, void *read_buf, long read_len)

// Rename and/or move a file to another path, optionally relative to one or two directories.
// If `*_at` is -1, the respective `*_path` is relative to the working directory.
// Returns -errno on error, 0 on success.
SYSCALL_DEF(21, SYSCALL_FS_RENAME, syscall_fs_rename, int, int old_at, char const *old_path, int new_at, char const *new_path, uint32_t flags)
    
// Get file status given file handler or path, optionally following the final symlink.
// If `path` is specified, it is interpreted as relative to the working directory.
// If both `path` and `fd` are specified, `path` is relative to the directory that `fd` describes.
// If only `fd` is specified, the inode referenced by `fd` is stat'ed.
// If `follow_link` is false, the last symlink in the path is not followed.
SYSCALL_DEF(22, SYSCALL_FS_STAT, syscall_fs_stat, int, int fd, char const *path, bool follow_link, stat_t *stat_out)

// Create a new directory.
// If `at` is -1, `path` is relative to the working directory.
// Returns -errno on error, 0 on success.
SYSCALL_DEF(47, SYSCALL_FS_MKDIR, syscall_fs_mkdir, int, int at, char const *path)

// Delete a directory if it is empty.
// If `at` is -1, `path` is relative to the working directory.
// Returns -errno on error, 0 on success.
SYSCALL_DEF(48, SYSCALL_FS_RMDIR, syscall_fs_rmdir, int, int at, char const *path)

// Create a new link to an existing inode.
// If `*_at` is -1, the respective `*_path` is relative to the working directory.
// Returns -errno on error, 0 on success.
SYSCALL_DEF(49, SYSCALL_FS_LINK, syscall_fs_link, int, int old_at, char const *old_path, int new_at, char const *new_path, uint32_t flags)

// Remove a link to an inode. If it is the last link, the file is deleted.
// If `at` is -1, `path` is relative to the working directory.
// Returns -errno on error, 0 on success.
SYSCALL_DEF(50, SYSCALL_FS_UNLINK, syscall_fs_unlink, int, int at, char const *path)

// Create a new FIFO / named pipe.
// If `at` is -1, `path` is relative to the working directory.
// Returns -errno on error, 0 on success.
SYSCALL_DEF(51, SYSCALL_FS_MKFIFO, syscall_fs_mkfifo, int, int at, char const *path)

// Create a new pipe.
// `fds[0]` will be written with the pointer to the read end, `fds[1]` the write end.
// Returns -errno on error, 0 on success.
SYSCALL_DEF(52, SYSCALL_FS_PIPE, syscall_fs_pipe, int, int fds[2], int flags)



/* ==== MEMORY MANAGEMENT SYSCALLS ==== */
// Implemented in process/syscall_impl.c

// Map a new range of memory at an arbitrary virtual address.
// This may round up to a multiple of the page size.
// Alignment may be less than `align` if the kernel doesn't support it.
SYSCALL_DEF(23, SYSCALL_MEM_MAP, syscall_mem_map, void *, void *address, size_t size, int prot, int flags)

// Unmap a range of memory previously allocated with `SYSCALL_MEM_MAP`.
// Returns whether a range of memory was unmapped.
SYSCALL_DEF_V(25, SYSCALL_MEM_UNMAP, syscall_mem_unmap, void *address, size_t size)



/* ==== SYSTEM MANAGEMENT SYSCALLS ==== */
// Implemented in main.c

// Start the shutdown process.
SYSCALL_DEF_V(45, SYSCALL_SYS_SHUTDOWN, syscall_sys_shutdown, bool is_reboot)



/* ==== TEMPORARY SYSCALLS ==== */
// Implemented in process/syscall_impl.c

// TEMPORARY: Write one or more characters to the log.
SYSCALL_DEF_V(46, SYSCALL_TEMP_WRITE, syscall_temp_write, char const *message, size_t length)



#undef SYSCALL_DEF
#undef SYSCALL_DEF_V
