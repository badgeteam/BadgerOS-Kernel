// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use crate::process::usercopy::UserCopyable;

/// A generic bit-set.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BitSet<const LENGTH: usize>(pub [usize; LENGTH.div_ceil(usize::BITS as usize)])
where
    [(); LENGTH.div_ceil(usize::BITS as usize)]:;

impl<const LENGTH: usize> BitSet<LENGTH>
where
    [(); LENGTH.div_ceil(usize::BITS as usize)]:,
{
    pub const WORD_LENGTH: usize = LENGTH.div_ceil(usize::BITS as usize);
    pub const EMPTY: Self = Self([0; _]);
    pub const FULL: Self = Self([usize::MAX; _]);

    pub fn invert(&mut self) {
        for i in 0..Self::WORD_LENGTH {
            self.0[i] = !self.0[i];
        }
    }

    pub fn add(&mut self, other: &Self) {
        for i in 0..Self::WORD_LENGTH {
            self.0[i] |= other.0[i];
        }
    }

    pub fn intersect(&mut self, other: &Self) {
        for i in 0..Self::WORD_LENGTH {
            self.0[i] &= other.0[i];
        }
    }

    pub fn subtract(&mut self, other: &Self) {
        for i in 0..Self::WORD_LENGTH {
            self.0[i] &= !other.0[i];
        }
    }

    pub fn test(&self, bit: usize) -> bool {
        let (word, bit) = (
            bit as usize / usize::BITS as usize,
            bit % usize::BITS as usize,
        );
        self.0[word] & (1 << bit) != 0
    }

    pub fn set(&mut self, bit: usize) {
        let (word, bit) = (
            bit as usize / usize::BITS as usize,
            bit % usize::BITS as usize,
        );
        self.0[word] |= 1 << bit;
    }

    pub fn clear(&mut self, bit: usize) {
        let (word, bit) = (
            bit as usize / usize::BITS as usize,
            bit % usize::BITS as usize,
        );
        self.0[word] &= !(1 << bit);
    }

    pub fn count(&self) -> usize {
        self.0.iter().map(|x| x.count_ones() as usize).sum()
    }
}

impl<const LENGTH: usize> Default for BitSet<LENGTH>
where
    [(); LENGTH.div_ceil(usize::BITS as usize)]:,
{
    fn default() -> Self {
        Self::EMPTY
    }
}

unsafe impl<const LENGTH: usize> UserCopyable for BitSet<LENGTH> where
    [(); LENGTH.div_ceil(usize::BITS as usize)]:
{
}
