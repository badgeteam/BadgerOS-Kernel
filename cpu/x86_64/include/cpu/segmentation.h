
// SPDX-License-Identifier: MIT

#pragma once

#ifndef __ASSEMBLER__
#include "attributes.h"

#include <stdint.h>



// GDT flag: long mode.
#define GDT_FLAG_L          (1llu << 53)
// GDT access: accessed bit, set to 1 when segment has been accessed.
#define GDT_ACCESS_A        (1llu << 40)
// GDT access: read/write, read-enable for code, write-enable for data.
#define GDT_ACCESS_RW       (1llu << 41)
/* DC access bit omitted - BadgerOS will never want to set it to 1. */
// GDT access: executable.
#define GDT_ACCESS_E        (1llu << 43)
// GDT access: is NOT a system descriptor.
#define GDT_ACCESS_S        (1llu << 44)
// GDT access: privilege level bitmask.
#define GDT_ACCESS_DPL_MASK (3llu << 45)
// GDT access: privilege level base bit.
#define GDT_ACCESS_DPL_BASE 45
// GDT access: present.
#define GDT_ACCESS_P        (1llu << 47)
// Format base address for GDT.
#define GDT_BASE(value)     ((((value) << 16) & 0x000000ffffff0000) | (((value) << 32) & 0xff00000000000000))
// Format limit for GDT.
#define GDT_LIMIT(value)    ((value) & 0xffff)
#endif

// Format a segment value without address.
#define FORMAT_SEGMENT(segno, use_local, privilege) (((segno) << 3) | ((use_local) << 2) | (privilege))

// Segment number to use for kernel code.
#define SEGNO_KCODE 1
// Segment number to use for kernel data.
#define SEGNO_KDATA 2
// Segment number to use for user code.
#define SEGNO_UCODE 3
// Segment number to use for user data.
#define SEGNO_UDATA 4

// Byte size of the TSS.
#define TSS_SIZE  0x68
// Offset of ring 0 stack in the TSS.
#define TSS_STACK 0x04
