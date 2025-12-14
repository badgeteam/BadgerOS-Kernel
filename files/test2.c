
#include "syscall.h"
#include "util.h"

int main(int argc, char **argv, char **envp) {

    syscall_thread_sleep(100000);

    printf("Hello, World! from /sbin/test2\n");

    return 0;
}
