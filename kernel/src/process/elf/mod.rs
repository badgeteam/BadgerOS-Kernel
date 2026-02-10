// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ptr::slice_from_raw_parts_mut;

use bytemuck_derive::{AnyBitPattern, NoUninit};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    config::{self, PAGE_SIZE},
    cpu,
    filesystem::File,
    mem::vmm::{self, Memmap},
    process::usercopy::UserSliceMut,
};

mod elf64;

/// ELF header magic.
pub const ELF_MAGIC: [u8; 4] = *b"\x7fELF";
/// The version of the ELF specification used.
pub const ELF_VERSION: u8 = 1;
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
pub const ELF_MACHINE: u16 = 243;
#[cfg(target_arch = "x86_64")]
pub const ELF_MACHINE: u16 = 62;

/// Auxiliary vector entry.
#[repr(C)]
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

/// Temporary mapping helper for [`load`].
fn map_helper(
    file: &dyn File,
    memmap: &Memmap,
    phdr: elf64::ProgHeader,
    load_offset: usize,
) -> EResult<()> {
    logkf!(
        LogLevel::Debug,
        "map_helper(..., ..., {:x?}, 0x{:x})",
        phdr,
        load_offset
    );

    let vaddr = phdr.vaddr as usize + load_offset;
    let vaddr_end = vaddr + phdr.mem_size as usize;

    unsafe {
        memmap.map_ram(
            Some(vaddr / PAGE_SIZE as usize),
            (vaddr_end - vaddr).div_ceil(PAGE_SIZE as usize),
            vmm::flags::RW,
        )?;
    }
    cpu::mmu::vmem_fence(None, None);

    let mut uslice = UserSliceMut::new_mut(vaddr as *mut u8, phdr.mem_size as usize)?;
    uslice.fill(0)?;
    file.seek_strong(phdr.offset, Errno::ENOEXEC)?;
    file.read(uslice.subslice_mut(0..phdr.file_size as usize))?;

    let mut prot = vmm::flags::R | vmm::flags::U;
    if phdr.flags & elf64::PF_W != 0 {
        prot |= vmm::flags::W;
    }
    if phdr.flags & elf64::PF_X != 0 {
        prot |= vmm::flags::X;
    }

    unsafe {
        memmap.protect(
            vaddr / PAGE_SIZE as usize,
            (vaddr_end - vaddr) / PAGE_SIZE as usize,
            prot,
        )?;
    }
    cpu::mmu::vmem_fence(None, None);

    Ok(())
}

/// Load an ELF file into a memory map.
/// Returns the entrypoint to jump to.
pub fn load(file: &dyn File, memmap: &Memmap, is_interp: bool) -> EResult<usize> {
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
            vmm::pagetable::canon_half_size() / 2
        } else {
            // 64K away from the start to have some margin without anything mapped.
            0x10000
        };
        base.wrapping_sub(min_vma_req)
    } else {
        0
    };

    for i in 0..header.phnum {
        file.seek_strong(
            header.phoff + i as u64 * header.phentsize as u64,
            Errno::ENOEXEC,
        )?;
        let phdr: elf64::ProgHeader = file.read_pod(Errno::ENOEXEC)?;

        if phdr.type_ as u32 == elf64::PT_LOAD {
            // TODO: This will be replaced with a proper file mapping in the future.
            map_helper(file, memmap, phdr, load_offset)?;
        }
    }

    Ok((header.entry as usize).wrapping_add(load_offset))
}
