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
    config::PAGE_SIZE,
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
    let start_page_fileoff = phdr.offset - phdr.offset % PAGE_SIZE as u64;
    let min_vaddr_real = (phdr.vaddr as usize).wrapping_add(load_offset);
    let max_vaddr_real =
        (phdr.vaddr.wrapping_add(phdr.mem_size) as usize).wrapping_add(load_offset);
    let min_vpn_real = min_vaddr_real / PAGE_SIZE as usize;
    let max_vpn_real = max_vaddr_real.div_ceil(PAGE_SIZE as usize);
    let mut flags = vmm::flags::R;
    if phdr.flags & elf64::PF_W != 0 {
        flags |= vmm::flags::W;
    }
    if phdr.flags & elf64::PF_X != 0 {
        flags |= vmm::flags::X;
    }

    let page_count = max_vpn_real - min_vpn_real;
    logkf!(
        LogLevel::Debug,
        "Mapping 0x{:x} bytes at 0x{:x}",
        page_count * PAGE_SIZE as usize,
        min_vpn_real * PAGE_SIZE as usize
    );
    unsafe { memmap.map_ram(Some(min_vpn_real), page_count, flags)? };

    let mut page_offset = 0;
    while page_offset < page_count {
        let mapping = memmap.virt2phys((min_vpn_real + page_offset) * PAGE_SIZE as usize);
        logkf!(
            LogLevel::Debug,
            "Reading 0x{:x} bytes into 0x{:x} (paddr 0x{:x})",
            mapping.size,
            mapping.page_vaddr,
            mapping.page_paddr
        );
        let hhdm_vaddr = mapping.page_paddr + unsafe { vmm::HHDM_OFFSET };
        let hhdm_slice =
            unsafe { &mut *slice_from_raw_parts_mut(hhdm_vaddr as *mut u8, mapping.size) };
        file.seek_strong(
            start_page_fileoff + page_offset as u64 * PAGE_SIZE as u64,
            Errno::ENOEXEC,
        )?;
        file.read(UserSliceMut::new_kernel_mut(hhdm_slice))?;
        page_offset += mapping.size / PAGE_SIZE as usize;
    }

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
