// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use super::CPU_SET_LEN;

/// Atomic bitset that can represent any usable CPU.
pub struct CpuSet(pub [u32; CPU_SET_LEN]);

impl CpuSet {
    pub const EMPTY: Self = Self([0; _]);
    pub const FULL: Self = Self([u32::MAX; _]);

    pub fn test(&self, cpu: u32) -> bool {
        let (word, bit) = (cpu as usize / 32, cpu % 32);
        self.0[word] & (1 << bit) != 0
    }

    pub fn set(&mut self, cpu: u32) {
        let (word, bit) = (cpu as usize / 32, cpu % 32);
        self.0[word] |= 1 << bit;
    }

    pub fn clear(&mut self, cpu: u32) {
        let (word, bit) = (cpu as usize / 32, cpu % 32);
        self.0[word] &= !(1 << bit);
    }

    pub fn count(&self) -> u32 {
        self.0.iter().map(|x| x.count_ones()).sum()
    }
}
