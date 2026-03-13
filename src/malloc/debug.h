#pragma once

#define BADGEROS_MALLOC_DEBUG_NONE  0
#define BADGEROS_MALLOC_DEBUG_ERROR 1
#define BADGEROS_MALLOC_DEBUG_WARN  2
#define BADGEROS_MALLOC_DEBUG_INFO  3
#define BADGEROS_MALLOC_DEBUG_DEBUG 4

#ifndef BADGEROS_MALLOC_DEBUG_LEVEL
#define BADGEROS_MALLOC_DEBUG_LEVEL BADEROS_MALLOC_DEBUG_NONE
#endif

#ifndef BADGEROS_KERNEL
#define _GNU_SOURCE
#include <stdarg.h>
#include <stdio.h>

#include <signal.h>
#include <unistd.h>

#define FMT_I  "%i"
#define FMT_ZI "%zi"
#define FMT_S  "%s"
#define FMT_D  "%d"
#define FMT_P  "%p"

#define BADGEROS_MALLOC_ASSERT(cond, level, format, ...)                                                               \
    do {                                                                                                               \
        if (!(cond)) {                                                                                                 \
            fprintf(                                                                                                   \
                stderr,                                                                                                \
                FMT_I ": %-7s: " FMT_S " " FMT_S "():" FMT_D ": Assertion failed: " format "\n",                       \
                gettid(),                                                                                              \
                level,                                                                                                 \
                __FILE__,                                                                                              \
                __func__,                                                                                              \
                __LINE__,                                                                                              \
                ##__VA_ARGS__                                                                                          \
            );                                                                                                         \
            raise(SIGINT);                                                                                             \
        }                                                                                                              \
    } while (0)


#define BADGEROS_MALLOC_DEBUG_MSG(level, format, ...)                                                                  \
    do {                                                                                                               \
        fprintf(                                                                                                       \
            stderr,                                                                                                    \
            FMT_I ": %-7s: " FMT_S " " FMT_S "(): " FMT_D ": " format "\n",                                            \
            gettid(),                                                                                                  \
            level,                                                                                                     \
            __FILE__,                                                                                                  \
            __func__,                                                                                                  \
            __LINE__,                                                                                                  \
            ##__VA_ARGS__                                                                                              \
        );                                                                                                             \
    } while (0)

#define BADGEROS_MALLOC_ASSERT_ERROR(cond, format, ...) BADGEROS_MALLOC_ASSERT(cond, "ERROR", format, ##__VA_ARGS__)
#define BADGEROS_MALLOC_MSG_ERROR(format, ...)          BADGEROS_MALLOC_DEBUG_MSG("ERROR", format, ##__VA_ARGS__)

#if BADGEROS_MALLOC_DEBUG_LEVEL >= BADGEROS_MALLOC_DEBUG_WARN
#define BADGEROS_MALLOC_ASSERT_WARN(cond, format, ...) BADGEROS_MALLOC_ASSERT(cond, "WARNING", format, ##__VA_ARGS__)
#define BADGEROS_MALLOC_MSG_WARN(format, ...)          BADGEROS_MALLOC_DEBUG_MSG("WARNING", format, ##__VA_ARGS__)
#endif

#if BADGEROS_MALLOC_DEBUG_LEVEL >= BADGEROS_MALLOC_DEBUG_INFO
#define BADGEROS_MALLOC_ASSERT_INFO(cond, format, ...) BADGEROS_MALLOC_ASSERT(cond, "INFO", format, ##__VA_ARGS__)
#define BADGEROS_MALLOC_MSG_INFO(format, ...)          BADGEROS_MALLOC_DEBUG_MSG("INFO", format, ##__VA_ARGS__)
#endif

#if BADGEROS_MALLOC_DEBUG_LEVEL >= BADGEROS_MALLOC_DEBUG_DEBUG
#define BADGEROS_MALLOC_ASSERT_DEBUG(cond, format, ...) BADGEROS_MALLOC_ASSERT(cond, "DEBUG", format, ##__VA_ARGS__)
#define BADGEROS_MALLOC_MSG_DEBUG(format, ...)          BADGEROS_MALLOC_DEBUG_MSG("DEBUG", format, ##__VA_ARGS__)
#endif

#else

#include "log.h"
#include "panic.h"

#include <stdarg.h>

#define FMT_I  "%{d}"
#define FMT_ZI "%{size;d}"
#define FMT_S  "%{cs}"
#define FMT_D  "%{d}"
#define FMT_P  "%{size;x}"

#define BADGEROS_MALLOC_ASSERT(cond, level, format, ...)                                                               \
    do {                                                                                                               \
        if (!(cond)) {                                                                                                 \
            logkf(                                                                                                     \
                level,                                                                                                 \
                FMT_S " " FMT_S "():" FMT_D ": Assertion failed: " format,                                             \
                __FILE__,                                                                                              \
                __func__,                                                                                              \
                __LINE__,                                                                                              \
                ##__VA_ARGS__                                                                                          \
            );                                                                                                         \
            panic_abort();                                                                                             \
        }                                                                                                              \
    } while (0)


#define BADGEROS_MALLOC_DEBUG_MSG(level, format, ...)                                                                  \
    do {                                                                                                               \
        logkf(level, FMT_S "():" FMT_D ": " format, __func__, __LINE__, ##__VA_ARGS__);                                \
    } while (0)

#define BADGEROS_MALLOC_ASSERT_ERROR(cond, format, ...) BADGEROS_MALLOC_ASSERT(cond, LOG_FATAL, format, ##__VA_ARGS__)
#define BADGEROS_MALLOC_MSG_ERROR(format, ...)          BADGEROS_MALLOC_DEBUG_MSG(LOG_FATAL, format, ##__VA_ARGS__)

#if BADGEROS_MALLOC_DEBUG_LEVEL >= BADGEROS_MALLOC_DEBUG_WARN
#define BADGEROS_MALLOC_ASSERT_WARN(cond, format, ...) BADGEROS_MALLOC_ASSERT(cond, LOG_WARN, format, ##__VA_ARGS__)
#define BADGEROS_MALLOC_MSG_WARN(format, ...)          BADGEROS_MALLOC_DEBUG_MSG(LOG_WARN, format, ##__VA_ARGS__)
#endif

#if BADGEROS_MALLOC_DEBUG_LEVEL >= BADGEROS_MALLOC_DEBUG_INFO
#define BADGEROS_MALLOC_ASSERT_INFO(cond, format, ...) BADGEROS_MALLOC_ASSERT(cond, LOG_INFO, format, ##__VA_ARGS__)
#define BADGEROS_MALLOC_MSG_INFO(format, ...)          BADGEROS_MALLOC_DEBUG_MSG(LOG_INFO, format, ##__VA_ARGS__)
#endif

#if BADGEROS_MALLOC_DEBUG_LEVEL >= BADGEROS_MALLOC_DEBUG_DEBUG
#define BADGEROS_MALLOC_ASSERT_DEBUG(cond, format, ...) BADGEROS_MALLOC_ASSERT(cond, LOG_DEBUG, format, ##__VA_ARGS__)
#define BADGEROS_MALLOC_MSG_DEBUG(format, ...)          BADGEROS_MALLOC_DEBUG_MSG(LOG_DEBUG, format, ##__VA_ARGS__)
#endif

#endif

// Define all of the macros in case they were not defined above

#ifndef BADGEROS_MALLOC_ASSERT_ERROR
#define BADGEROS_MALLOC_ASSERT_ERROR(cond, format, ...)
#endif
#ifndef BADGEROS_MALLOC_ASSERT_WARN
#define BADGEROS_MALLOC_ASSERT_WARN(cond, format, ...)
#endif
#ifndef BADGEROS_MALLOC_ASSERT_INFO
#define BADGEROS_MALLOC_ASSERT_INFO(cond, format, ...)
#endif
#ifndef BADGEROS_MALLOC_ASSERT_DEBUG
#define BADGEROS_MALLOC_ASSERT_DEBUG(cond, format, ...)
#endif

#ifndef BADGEROS_MALLOC_MSG_ERROR
#define BADGEROS_MALLOC_MSG_ERROR(format, ...)
#endif
#ifndef BADGEROS_MALLOC_MSG_WARN
#define BADGEROS_MALLOC_MSG_WARN(format, ...)
#endif
#ifndef BADGEROS_MALLOC_MSG_INFO
#define BADGEROS_MALLOC_MSG_INFO(format, ...)
#endif
#ifndef BADGEROS_MALLOC_MSG_DEBUG
#define BADGEROS_MALLOC_MSG_DEBUG(format, ...)
#endif
