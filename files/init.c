
#include "syscall.h"
#include "util.h"

#include <signal.h>

int main(int argc, char **argv, char **envp) {
    printf("Hello World from /sbin/init!\n");

    int res = syscall_proc_fork();

    printf("fork() returned %d\n", res);

    return 0;
}
