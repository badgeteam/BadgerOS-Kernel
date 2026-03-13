// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use bytemuck_derive::{AnyBitPattern, NoUninit};

use super::ElfIdent;

/// No file type.
pub const ET_NONE: u32 = 0;
/// Relocatable file.
pub const ET_REL: u32 = 1;
/// Executable file.
pub const ET_EXEC: u32 = 2;
/// Shared object file.
pub const ET_DYN: u32 = 3;
/// Core file.
pub const ET_CORE: u32 = 4;
/// Operating system-specific.
pub const ET_LOOS: u32 = 0xfe00;
/// Operating system-specific.
pub const ET_HIOS: u32 = 0xfeff;
/// Processor-specific.
pub const ET_LOPROC: u32 = 0xff00;
/// Processor-specific.
pub const ET_HIPROC: u32 = 0xffff;

/// ELF file header.
#[repr(C)]
#[derive(Clone, Copy, NoUninit, AnyBitPattern, Default)]
pub struct ElfHeader {
    /// ELF file identifier.
    pub ident: ElfIdent,
    /// What type of ELF file this is.
    pub type_: u16,
    /// What machine this binary targets.
    pub machine: u16,
    /// Must contain [`super::ELF_VERSION`].
    pub version: u32,
    /// Entrypoint address.
    pub entry: u64,
    /// Program header table offset.
    pub phoff: u64,
    /// Section header table offset.
    pub shoff: u64,
    /// Processor-specific flags.
    pub flags: u32,
    /// Size of this header.
    pub ehsize: u16,
    /// Size of a program header entry.
    pub phentsize: u16,
    /// Number of program header entries.
    pub phnum: u16,
    /// Size of a section header entry.
    pub shentsize: u16,
    /// Number of section header entries.
    pub shnum: u16,
    /// Index in the section header table of the section header string table.
    pub shstrndx: u16,
}

/// Unused program header.
pub const PT_NULL: u32 = 0;
/// Loadable segment.
pub const PT_LOAD: u32 = 1;
/// Dynamic linking information table.
pub const PT_DYNAMIC: u32 = 2;
/// Dynamic executable interpreter.
pub const PT_INTERP: u32 = 3;
/// Auxiliary information.
pub const PT_NOTE: u32 = 4;
/// Describes the address of the program header table itself.
/// May only be present if the program headers are loaded themselves, in which case it must precede loadable segments.
pub const PT_PHDR: u32 = 6;
/// The thread-local storage template.
pub const PT_TLS: u32 = 7;
pub const PT_LOOS: u32 = 0x60000000;
pub const PT_HIOS: u32 = 0x6fffffff;
pub const PT_LOPROC: u32 = 0x70000000;
pub const PT_HIPROC: u32 = 0x7fffffff;

pub const PF_X: u32 = 0x1;
pub const PF_W: u32 = 0x2;
pub const PF_R: u32 = 0x4;
pub const PF_MASKOS: u32 = 0x0ff00000;
pub const PF_MASKPROC: u32 = 0xf0000000;

/// Program header; contains information about how to load the ELF file.
#[repr(C)]
#[derive(Debug, Clone, Copy, NoUninit, AnyBitPattern, Default)]
pub struct ProgHeader {
    pub type_: u32,
    pub flags: u32,
    pub offset: u64,
    pub vaddr: u64,
    pub paddr: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub align: u64,
}
