// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering};

use super::CPU_SET_LEN;

/// Atomic bitset that can represent any usable CPU.
pub struct AtomicCpuSet(pub [AtomicU32; CPU_SET_LEN]);

impl AtomicCpuSet {
    pub const EMPTY: Self = Self([AtomicU32::new(0); _]);
    pub const FULL: Self = Self([AtomicU32::new(u32::MAX); _]);

    pub fn test(&self, cpu: u32) -> bool {
        let (word, bit) = (cpu as usize / 32, cpu % 32);
        self.0[word].load(Ordering::Relaxed) & (1 << bit) != 0
    }

    pub fn test_and_set(&self, cpu: u32) -> bool {
        let (word, bit) = (cpu as usize / 32, cpu % 32);
        self.0[word].fetch_or(1 << bit, Ordering::Relaxed) & (1 << bit) != 0
    }

    pub fn test_and_clear(&self, cpu: u32) -> bool {
        let (word, bit) = (cpu as usize / 32, cpu % 32);
        self.0[word].fetch_and(!(1 << bit), Ordering::Relaxed) & (1 << bit) != 0
    }

    pub fn racy_set_all(&self) {
        for x in &self.0 {
            x.store(u32::MAX, Ordering::Relaxed);
        }
    }

    pub fn racy_clear_all(&self) {
        for x in &self.0 {
            x.store(0, Ordering::Relaxed);
        }
    }

    pub fn racy_count(&self) -> u32 {
        self.0
            .iter()
            .map(|x| x.load(Ordering::Relaxed).count_ones())
            .sum()
    }
}
