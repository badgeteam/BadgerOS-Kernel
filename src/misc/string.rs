// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

// This entire file serves to implement unsafe code, no need to spam the keywords another 40 times.
#![allow(unsafe_op_in_unsafe_fn)]

use core::mem::MaybeUninit;

/// If `true`, assume that unaligned accesses are as fast as aligned ones.
pub static mut ASSUME_FAST_UNALIGNED: bool = cfg_select! {
    target_arch = "x86_64" => {true}
    target_arch = "riscv64" => {false}
};

/// How many iterations will be unrolled inside `memcpy`.
pub const UNROLL_COUNT: usize = cfg_select! {
    target_arch = "x86_64" => {8}
    target_arch = "riscv64" => {16}
};

#[inline(always)]
unsafe fn ptr_read<T: Copy>(ptr: *const T) -> T {
    #[cfg(not(test))]
    return core::intrinsics::volatile_load(ptr);
    #[cfg(test)]
    return std::intrinsics::volatile_load(ptr);
}

#[inline(always)]
unsafe fn ptr_write<T: Copy>(ptr: *mut T, value: T) {
    #[cfg(not(test))]
    core::intrinsics::volatile_store(ptr, value);
    #[cfg(test)]
    std::intrinsics::volatile_store(ptr, value);
}

unsafe fn memcpy_fwd_spec_impl<T: Copy>(dest: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    let mut index = 0;

    let dest = dest as *mut T;
    let src = src as *const T;

    while index + UNROLL_COUNT * size_of::<T>() <= len {
        let mut tmp = [const { MaybeUninit::<T>::uninit() }; UNROLL_COUNT];

        for i in 0..UNROLL_COUNT {
            tmp[i] = MaybeUninit::new(ptr_read(src.wrapping_byte_add(index + i * size_of::<T>())));
        }
        for i in 0..UNROLL_COUNT {
            ptr_write(
                dest.wrapping_byte_add(index + i * size_of::<T>()),
                tmp[i].assume_init_read(),
            );
        }

        index += UNROLL_COUNT * size_of::<T>();
    }

    if size_of::<T>() > 1 {
        while index + size_of::<T>() <= len {
            let val0 = ptr_read(src.wrapping_byte_add(index));
            ptr_write(dest.wrapping_byte_add(index), val0);
            index += size_of::<T>();
        }
    }

    let dest = dest as *mut u8;
    let src = src as *const u8;

    while index < len {
        let val0 = ptr_read(src.wrapping_byte_add(index));
        ptr_write(dest.wrapping_byte_add(index), val0);
        index += 1;
    }

    dest
}

unsafe fn memcpy_rev_spec_impl<T: Copy>(dest: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    let mut index = len;

    while index > len & !(size_of::<T>() - 1) {
        index -= 1;
        let val0 = ptr_read(src.wrapping_byte_add(index));
        ptr_write(dest.wrapping_byte_add(index), val0);
    }

    let dest = dest as *mut T;
    let src = src as *const T;

    if size_of::<T>() > 1 {
        while index > len & !(UNROLL_COUNT * size_of::<T>() - 1) {
            index -= size_of::<T>();
            let val0 = ptr_read(src.wrapping_byte_add(index));
            ptr_write(dest.wrapping_byte_add(index), val0);
        }
    }

    while index > 0 {
        index -= UNROLL_COUNT * size_of::<T>();
        let mut tmp = [const { MaybeUninit::<T>::uninit() }; UNROLL_COUNT];

        for i in 0..UNROLL_COUNT {
            tmp[i] = MaybeUninit::new(ptr_read(src.wrapping_byte_add(index + i * size_of::<T>())));
        }
        for i in 0..UNROLL_COUNT {
            ptr_write(
                dest.wrapping_byte_add(index + i * size_of::<T>()),
                tmp[i].assume_init_read(),
            );
        }
    }

    dest as *mut u8
}

unsafe fn memcpy_spec_impl<T: Copy, const REVERSE: bool>(
    dest: *mut u8,
    src: *const u8,
    len: usize,
) -> *mut u8 {
    if REVERSE {
        memcpy_rev_spec_impl::<T>(dest, src, len)
    } else {
        memcpy_fwd_spec_impl::<T>(dest, src, len)
    }
}

unsafe fn memcpy_impl<const REVERSE: bool>(dest: *mut u8, src: *const u8, len: usize) -> *const u8 {
    #[cfg(target_arch = "x86_64")]
    if len >= 512 {
        if REVERSE {
            core::arch::asm!(
                "std; rep movsb; cld",
                inout("rdi")dest as usize + len - 1 => _,
                inout("rsi")src as usize + len - 1 => _,
                inout("rcx")len => _,
            );
        } else {
            core::arch::asm!(
                "rep movsb",
                inout("rdi")dest => _,
                inout("rsi")src => _,
                inout("rcx")len => _,
            );
        }
        return dest;
    }

    let align = dest as usize | src as usize;
    if ASSUME_FAST_UNALIGNED || align & 7 == 0 {
        memcpy_spec_impl::<u64, REVERSE>(dest, src, len)
    } else if align & 3 == 0 {
        memcpy_spec_impl::<u32, REVERSE>(dest, src, len)
    } else if align & 1 == 0 {
        memcpy_spec_impl::<u16, REVERSE>(dest, src, len)
    } else {
        memcpy_spec_impl::<u8, REVERSE>(dest, src, len)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, len: usize) -> *const u8 {
    memcpy_impl::<false>(dest, src, len)
}

unsafe fn memcmp_spec_impl<T: Copy + Eq + Ord>(lhs: *const u8, rhs: *const u8, len: usize) -> i32 {
    let mut index = 0;

    let lhs = lhs as *const T;
    let rhs = rhs as *const T;

    while index + UNROLL_COUNT * size_of::<T>() <= len {
        let mut acc = false;
        for i in 0..UNROLL_COUNT {
            acc |= ptr_read(lhs.wrapping_byte_add(index + i * size_of::<T>()))
                != ptr_read(rhs.wrapping_byte_add(index + i * size_of::<T>()));
        }
        if acc {
            break;
        }

        index += UNROLL_COUNT * size_of::<T>();
    }

    if size_of::<T>() > 1 {
        while index + size_of::<T>() <= len {
            if ptr_read(lhs.wrapping_byte_add(index)) != ptr_read(rhs.wrapping_byte_add(index)) {
                break;
            }
            index += size_of::<T>();
        }
    }

    let lhs = lhs as *const u8;
    let rhs = rhs as *const u8;

    while index < len {
        let res = ptr_read(lhs.wrapping_byte_add(index)) as i32
            - ptr_read(rhs.wrapping_byte_add(index)) as i32;
        if res != 0 {
            return res;
        }
        index += 1;
    }

    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(lhs: *const u8, rhs: *const u8, len: usize) -> i32 {
    #[cfg(target_arch = "x86_64")]
    if len >= 512 {
        let cmp;
        core::arch::asm!(
            "
            rep cmpsb
            jl 2f
            jg 3f
            mov rax, 0
            j 4f
        2:
            mov rax, -1
            j 4f
        3:
            mov rax, 1
            j 4f
        4:
            ",
            inout("rdi") lhs => _,
            inout("rsi") rhs => _,
            inout("ecx") len => _,
            out("rax") cmp,
        );
        return cmp;
    }

    let align = lhs as usize | rhs as usize;
    if ASSUME_FAST_UNALIGNED || align & 7 == 0 {
        memcmp_spec_impl::<u64>(lhs, rhs, len)
    } else if align & 3 == 0 {
        memcmp_spec_impl::<u32>(lhs, rhs, len)
    } else if align & 1 == 0 {
        memcmp_spec_impl::<u16>(lhs, rhs, len)
    } else {
        memcmp_spec_impl::<u8>(lhs, rhs, len)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, len: usize) -> *const u8 {
    if (dest as *const u8) > src && src.wrapping_byte_add(len) > (dest as *const u8) {
        memcpy_impl::<true>(dest, src, len)
    } else if dest as *const u8 != src {
        memcpy_impl::<false>(dest, src, len)
    } else {
        dest
    }
}

unsafe fn memset_spec_impl<T: Copy>(dest: *mut u8, val: T, val_u8: u8, len: usize) -> *const u8 {
    let mut index = 0;

    let dest = dest as *mut T;

    while index + UNROLL_COUNT * size_of::<T>() <= len {
        for i in 0..UNROLL_COUNT {
            ptr_write(dest.wrapping_byte_add(index + i * size_of::<T>()), val);
        }

        index += UNROLL_COUNT * size_of::<T>();
    }

    while index + size_of::<T>() <= len {
        ptr_write(dest.wrapping_byte_add(index), val);
        index += size_of::<T>();
    }

    let dest = dest as *mut u8;

    if size_of::<T>() > 1 {
        while index < len {
            ptr_write(dest.wrapping_byte_add(index), val_u8);
            index += 1;
        }
    }

    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dest: *mut u8, val: i32, len: usize) -> *const u8 {
    let val = val as u8;

    #[cfg(target_arch = "x86_64")]
    if len >= 512 {
        core::arch::asm!(
            "rep stosb",
            inout("rdi")dest => _,
            in("al")val,
            inout("rcx")len - 1 => _,
        );
        return dest;
    }

    let align = dest as usize;
    if ASSUME_FAST_UNALIGNED || align & 7 == 0 {
        memset_spec_impl::<u64>(dest, val as u64 * 0x01010101_01010101, val, len)
    } else if align & 3 == 0 {
        memset_spec_impl::<u32>(dest, val as u32 * 0x01010101, val, len)
    } else if align & 1 == 0 {
        memset_spec_impl::<u16>(dest, val as u16 * 0x0101, val, len)
    } else {
        memset_spec_impl::<u8>(dest, val as u8 * 0x01, val, len)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strlen(str: *const u8) -> usize {
    let mut ptr = str;
    while ptr_read(ptr) != 0 {
        ptr = ptr.wrapping_byte_add(1);
    }
    ptr.offset_from_unsigned(str)
}
