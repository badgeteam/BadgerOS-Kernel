// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::vec::Vec;
use bytemuck_derive::{AnyBitPattern, NoUninit};

use crate::{
    bindings::error::{EResult, Errno},
    config::PAGE_SIZE,
    filesystem::{self, File, oflags},
    mem::vmm::{
        self,
        map::{self, Mapping, VmSpace},
        prot,
    },
};

use super::usercopy::UserSliceMut;

mod elf64;

/// ELF header magic.
pub const ELF_MAGIC: [u8; 4] = *b"\x7fELF";
/// The version of the ELF specification used.
pub const ELF_VERSION: u8 = 1;
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
pub const ELF_MACHINE: u16 = 243;
#[cfg(target_arch = "x86_64")]
pub const ELF_MACHINE: u16 = 62;

pub const AT_NULL: usize = 0;
pub const AT_IGNORE: usize = 1;
pub const AT_EXECFD: usize = 2;
pub const AT_PHDR: usize = 3;
pub const AT_PHENT: usize = 4;
pub const AT_PHNUM: usize = 5;
pub const AT_PAGESZ: usize = 6;
pub const AT_BASE: usize = 7;
pub const AT_FLAGS: usize = 8;
pub const AT_ENTRY: usize = 9;
pub const AT_UID: usize = 11;
pub const AT_EUID: usize = 12;
pub const AT_GID: usize = 13;
pub const AT_EGID: usize = 14;

/// Auxiliary vector entry.
#[repr(C)]
#[derive(Clone, Copy, NoUninit, AnyBitPattern)]
pub struct AuxvEntry {
    pub type_: usize,
    pub value: usize,
}

/// Header that identifies a file as an ELF file.
#[repr(C)]
#[derive(Clone, Copy, NoUninit, AnyBitPattern, Default)]
pub struct ElfIdent {
    /// Must contain [`ELF_MAGIC`].
    pub magic: [u8; 4],
    /// File class; 1 for 32-bit, 2 for 64-bit.
    pub class: u8,
    /// File data encoding; 1 for little-endian, 2 for big-endian.
    pub endian: u8,
    /// Must contain [`ELF_VERSION`].
    pub version: u8,
    /// Operating system / ABI identification.
    pub osabi: u8,
    /// ABI version.
    pub abi_version: u8,
    /// Padding.
    pub _padding0: [u8; 7],
}

/// Program header segment mapping helper for [`load_impl`].
fn map_helper(
    file: &dyn File,
    memmap: &VmSpace,
    phdr: elf64::ProgHeader,
    load_offset: usize,
) -> EResult<()> {
    // Address calculations.
    let vaddr = phdr.vaddr as usize + load_offset;
    let end_vaddr = vaddr + phdr.file_size as usize;
    let file_start_vaddr = vaddr / PAGE_SIZE as usize * PAGE_SIZE as usize;
    let file_end_vaddr =
        (vaddr + phdr.file_size as usize).div_ceil(PAGE_SIZE as usize) * PAGE_SIZE as usize;
    let mem_end_vaddr =
        (vaddr + phdr.mem_size as usize).div_ceil(PAGE_SIZE as usize) * PAGE_SIZE as usize;
    let file_start_offset = phdr.offset - (vaddr - file_start_vaddr) as u64;

    // Mapping: Phase 1: Back with anonymous allocations.
    memmap.map(
        mem_end_vaddr - file_start_vaddr,
        file_start_vaddr,
        map::PRIVATE | map::FIXED,
        prot::READ | prot::WRITE,
        None,
    )?;

    // Mapping: Phase 2: Cover with file mappings.
    memmap.map(
        file_end_vaddr - file_start_vaddr,
        file_start_vaddr,
        map::PRIVATE | map::FIXED | map::DENYWRITE,
        prot::READ | prot::WRITE,
        Some(Mapping {
            offset: file_start_offset,
            object: file.get_memobject().ok_or(Errno::ENODEV)?,
        }),
    )?;

    // Mapping: Phase 3: Enforce all BSS is zeroes and write zeroes if it is not.
    let check_bss_zero = |mut uptr: UserSliceMut<u8>| -> EResult<()> {
        let mut write = false;
        for i in 0..uptr.len() {
            if write || uptr.read(i)? != 0 {
                write = true;
                uptr.write(i, 0)?;
            }
        }
        Ok(())
    };

    check_bss_zero(UserSliceMut::new_mut(
        file_start_vaddr as *mut u8,
        vaddr - file_start_vaddr,
    )?)?;
    check_bss_zero(UserSliceMut::new_mut(
        end_vaddr as *mut u8,
        file_end_vaddr - end_vaddr,
    )?)?;

    // Mapping: Phase 4: Apply protections.
    let mut prot = 0;
    if phdr.flags & elf64::PF_R != 0 {
        prot |= prot::READ;
    }
    if phdr.flags & elf64::PF_W != 0 {
        // On most architectures, and in BadgerOS, write implies read.
        prot |= prot::WRITE | prot::READ;
    }
    if phdr.flags & elf64::PF_X != 0 {
        prot |= prot::EXEC;
    }
    memmap.protect(file_start_vaddr..mem_end_vaddr, prot)?;

    Ok(())
}

/// Temporary mapping helper for [`load`].
fn map_helper1(
    file: &dyn File,
    memmap: &VmSpace,
    phdr: elf64::ProgHeader,
    load_offset: usize,
) -> EResult<()> {
    let vaddr = phdr.vaddr as usize + load_offset;
    let aligned_vaddr = vaddr / PAGE_SIZE as usize * PAGE_SIZE as usize;
    let vaddr_end = vaddr + phdr.mem_size as usize;
    let aligned_offset = phdr.offset / PAGE_SIZE as u64 * PAGE_SIZE as u64;

    if vaddr % PAGE_SIZE as usize != phdr.offset as usize % PAGE_SIZE as usize {
        return Err(Errno::ENOEXEC);
    }

    let mut prot = prot::READ;
    if phdr.flags & elf64::PF_W != 0 {
        prot |= prot::WRITE;
    }
    if phdr.flags & elf64::PF_X != 0 {
        prot |= prot::EXEC;
    }

    memmap.map(
        vaddr_end - aligned_vaddr,
        aligned_vaddr,
        vmm::map::FIXED | vmm::map::PRIVATE | vmm::map::DENYWRITE | vmm::map::POPULATE,
        prot,
        Some(Mapping {
            offset: aligned_offset,
            object: file.get_memobject().ok_or(Errno::ENODEV)?,
        }),
    )?;

    Ok(())
}

/// Load an ELF file into a memory map.
/// Returns the entrypoint to jump to.
pub fn load(file: &dyn File, memmap: &VmSpace, auxv: &mut Vec<AuxvEntry>) -> EResult<usize> {
    let (entry, _) = load_impl(file, memmap, auxv, false)?;
    Ok(entry)
}

/// Returns `(entry, load_offset)`.
pub fn load_impl(
    file: &dyn File,
    memmap: &VmSpace,
    auxv: &mut Vec<AuxvEntry>,
    is_interp: bool,
) -> EResult<(usize, usize)> {
    file.seek_strong(0, Errno::ENOEXEC)?;
    let header: elf64::ElfHeader = file.read_pod(Errno::ENOEXEC)?;

    // Validate the ELF header.
    if !(header.ident.magic == ELF_MAGIC
        && header.ident.version == ELF_VERSION
        && header.ident.class == 2
        && header.ident.endian == 1
        && header.version == ELF_VERSION as u32
        && header.machine == ELF_MACHINE)
    {
        return Err(Errno::ENOEXEC);
    }

    let mut min_vma_req = usize::MAX;
    let mut max_vma_req = 0usize;
    for i in 0..header.phnum {
        file.seek_strong(
            header.phoff + i as u64 * header.phentsize as u64,
            Errno::ENOEXEC,
        )?;
        let phdr: elf64::ProgHeader = file.read_pod(Errno::ENOEXEC)?;

        if phdr.type_ as u32 == elf64::PT_LOAD {
            if phdr.file_size > 0 && phdr.vaddr % PAGE_SIZE as u64 != phdr.offset % PAGE_SIZE as u64
            {
                return Err(Errno::ENOEXEC);
            }
            min_vma_req = min_vma_req.min(phdr.vaddr as usize);
            max_vma_req = max_vma_req.max(phdr.vaddr as usize + phdr.mem_size as usize);
        }
    }

    let load_offset = if header.type_ == elf64::ET_DYN as u16 {
        // Decide the best address to load.
        let base = if is_interp {
            // Halfway through virtual memory to avoid getting in user code's way.
            vmm::physmap::canon_half_size() / 2
        } else {
            // 64K away from the start to have some margin without anything mapped.
            0x10000
        };
        base.wrapping_sub(min_vma_req)
    } else {
        0
    };

    let mut entry = (header.entry as usize).wrapping_add(load_offset);
    auxv.push(AuxvEntry {
        type_: AT_ENTRY,
        value: entry,
    });

    for i in 0..header.phnum {
        file.seek_strong(
            header.phoff + i as u64 * header.phentsize as u64,
            Errno::ENOEXEC,
        )?;
        let phdr: elf64::ProgHeader = file.read_pod(Errno::ENOEXEC)?;

        if phdr.type_ as u32 == elf64::PT_LOAD {
            // TODO: This will be replaced with a proper file mapping in the future.
            map_helper(file, memmap, phdr, load_offset)?;
        } else if phdr.type_ as u32 == elf64::PT_PHDR {
            auxv.push(AuxvEntry {
                type_: AT_PHENT,
                value: header.phentsize as _,
            });
            auxv.push(AuxvEntry {
                type_: AT_PHNUM,
                value: header.phnum as _,
            });
            auxv.push(AuxvEntry {
                type_: AT_PHDR,
                value: phdr.vaddr as usize + load_offset,
            });
        } else if phdr.type_ as u32 == elf64::PT_INTERP {
            if is_interp {
                return Err(Errno::ENOEXEC);
            }

            let mut path = Vec::try_with_capacity(phdr.mem_size as _)?;
            path.resize(phdr.file_size as _, 0);
            file.seek_strong(phdr.offset, Errno::ENOEXEC)?;
            file.readk(&mut path)?;
            path.resize(phdr.mem_size as _, 0);
            let len = path
                .iter()
                .enumerate()
                .find(|x| *x.1 == 0)
                .map(|x| x.0)
                .unwrap_or(path.len());
            path.resize(len, 0);
            let interp_file = filesystem::open(None, &path, oflags::READ_ONLY | oflags::FILE_ONLY)?;

            let mut dummy = Vec::new();
            let (interp_entry, interp_base) =
                load_impl(interp_file.as_ref(), memmap, &mut dummy, true)?;
            entry = interp_entry;
            auxv.push(AuxvEntry {
                type_: AT_BASE,
                value: interp_base,
            });
        }
    }

    Ok((entry, load_offset))
}
