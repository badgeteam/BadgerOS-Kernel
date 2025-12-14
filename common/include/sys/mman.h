
// SPDX-License-Identifier: MIT

#pragma once

#include <stddef.h>

#include <sys/types.h>

#define MAP_FAILED ((void *)-1)

#define PROT_NONE  0
#define PROT_READ  1
#define PROT_WRITE 2
#define PROT_EXEC  4

#define MAP_SHARED   1
#define MAP_PRIVATE  2
#define MAP_ANON     4
#define MAP_FIXED    8
#define MAP_POPULATE 16

void *mmap(void *addr, size_t length, int prot, int flags, int fd, off_t offset);
int   munmap(void *addr, size_t length);
