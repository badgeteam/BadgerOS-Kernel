
// SPDX-License-Identifier: MIT

#pragma once

#include "log.h"

#define KBELF_DEBUG

#ifndef KBELF_FMT_CSTR
#define KBELF_FMT_CSTR "%{cs}"
#endif

#ifndef KBELF_FMT_DEC
#define KBELF_FMT_DEC "%{d}"
#endif

#ifndef KBELF_FMT_SIZE
#define KBELF_FMT_SIZE "%{size;d}"
#endif

#ifndef KBELF_FMT_BYTE
#define KBELF_FMT_BYTE "%{u8;x}"
#endif

#ifndef KBELF_LOGGER
#define KBELF_LOGGER(lv, fmt, ...) logkf(lv, fmt __VA_OPT__(, ) __VA_ARGS__)
#endif

#ifndef KBELF_LOG_LEVEL_D
#define KBELF_LOG_LEVEL_D LOG_DEBUG
#endif

#ifndef KBELF_LOG_LEVEL_I
#define KBELF_LOG_LEVEL_I LOG_INFO
#endif

#ifndef KBELF_LOG_LEVEL_W
#define KBELF_LOG_LEVEL_W LOG_WARN
#endif

#ifndef KBELF_LOG_LEVEL_E
#define KBELF_LOG_LEVEL_E LOG_ERROR
#endif