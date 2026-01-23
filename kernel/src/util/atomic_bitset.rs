// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    sync::atomic::{AtomicUsize, Ordering},
    usize,
};

/// A generic atomic bit-set.
pub struct AtomicBitSet<const LENGTH: usize>(
    pub [AtomicUsize; LENGTH.div_ceil(usize::BITS as usize)],
)
where
    [(); LENGTH.div_ceil(usize::BITS as usize)]:;

impl<const LENGTH: usize> AtomicBitSet<LENGTH>
where
    [(); LENGTH.div_ceil(usize::BITS as usize)]:,
{
    pub const WORD_LENGTH: usize = LENGTH.div_ceil(usize::BITS as usize);
    pub const EMPTY: Self = Self([const { AtomicUsize::new(0) }; _]);
    pub const FULL: Self = Self([const { AtomicUsize::new(usize::MAX) }; _]);

    pub fn test(&self, bit: usize) -> bool {
        let (word, bit) = (
            bit as usize / usize::BITS as usize,
            bit % usize::BITS as usize,
        );
        self.0[word].load(Ordering::Relaxed) & (1 << bit) != 0
    }

    pub fn test_and_set(&self, bit: usize) -> bool {
        let (word, bit) = (
            bit as usize / usize::BITS as usize,
            bit % usize::BITS as usize,
        );
        self.0[word].fetch_or(1 << bit, Ordering::Relaxed) & (1 << bit) != 0
    }

    pub fn test_and_clear(&self, bit: usize) -> bool {
        let (word, bit) = (
            bit as usize / usize::BITS as usize,
            bit % usize::BITS as usize,
        );
        self.0[word].fetch_and(!(1 << bit), Ordering::Relaxed) & (1 << bit) != 0
    }

    pub fn racy_clear_all(&self) {
        for x in &self.0 {
            x.store(0, Ordering::Relaxed);
        }
    }

    pub fn racy_set_all(&self) {
        for x in &self.0 {
            x.store(usize::MAX, Ordering::Relaxed);
        }
    }

    pub fn racy_count(&self) -> usize {
        self.0
            .iter()
            .map(|x| x.load(Ordering::Relaxed).count_ones() as usize)
            .sum()
    }
}
