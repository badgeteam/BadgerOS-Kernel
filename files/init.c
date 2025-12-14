
#include "sys/mman.h"
#include "syscall.h"
#include "util.h"

#include <stddef.h>

#include <signal.h>

int main(int argc, char **argv, char **envp) {
    printf("Hello World from /sbin/init!\n");

    pid_t res = syscall_proc_fork();
    if (res == 0) {
        syscall_thread_sleep(100000);
    }

    printf("fork() returned %ld\n", res);

    if (res == 0) {
        return 0;
    }

    printf("Mapping prepopulated\n");
    char volatile *populated =
        syscall_mem_map(NULL, 2048, PROT_READ | PROT_WRITE, MAP_POPULATE | MAP_ANON | MAP_PRIVATE);
    if ((ptrdiff_t)populated < 0) {
        printf("MMAP failed: %d\n", (int)(ptrdiff_t)populated);
        return 1;
    }

    printf("Accessing prepopulated\n");
    *populated = 1;
    printf("Result: %d\n", *populated);

    printf("Mapping lazily\n");
    char volatile *lazy = syscall_mem_map(NULL, 2048, PROT_READ | PROT_WRITE, MAP_ANON | MAP_PRIVATE);
    if ((ptrdiff_t)lazy < 0) {
        printf("MMAP failed: %d\n", (int)(ptrdiff_t)lazy);
        return 1;
    }

    printf("Accessing lazily\n");
    *lazy = 1;
    printf("Result: %d\n", *lazy);

    printf("Exec'ing /sbin/test2\n");

    int exec_res = syscall_proc_exec("/sbin/test2", (char const *const[]){NULL}, NULL);

    printf("exec() returned %d\n", exec_res);

    return 0;
}
