// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ops::Deref;

use crate::cpu;

/// How many individual fences to perform before a big fence is considered better.
pub const BIG_FENCE_THRESHOLD: usize = 64;

/// Represents a collection of virtual-memory fences to perform all at once.
/// Automatically determines whether it is better to do individual fences or one big fence.
#[derive(Debug, Clone)]
pub struct VmFenceSet {
    fences: [usize; BIG_FENCE_THRESHOLD],
    len: usize,
}

impl VmFenceSet {
    pub const fn new() -> Self {
        Self {
            fences: [0; _],
            len: 0,
        }
    }

    /// Execute the fences described in this set.
    pub fn execute(&self) {
        if self.len == BIG_FENCE_THRESHOLD {
            cpu::mmu::vmem_fence(None, None);
        } else {
            for i in 0..self.len {
                cpu::mmu::vmem_fence(Some(self.fences[i]), None);
            }
        }
    }

    /// Add a single additional fence.
    pub fn add(&mut self, vaddr: Option<usize>, _asid: Option<usize>) {
        if let Some(vaddr) = vaddr
            && self.len != BIG_FENCE_THRESHOLD - 1
        {
            self.fences[self.len] = vaddr;
            self.len += 1;
        } else {
            self.len = BIG_FENCE_THRESHOLD;
        }
    }

    /// Add one or more additional fences from another [`VmFenceSet`].
    pub fn extend_from(&mut self, other: &Self) {
        if self.len + other.len >= BIG_FENCE_THRESHOLD {
            self.len = BIG_FENCE_THRESHOLD;
        } else {
            for i in 0..other.len {
                self.fences[self.len + i] = other.fences[i];
            }
            self.len += other.len;
        }
    }

    /// Clear this set.
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl Deref for VmFenceSet {
    type Target = [usize];

    fn deref(&self) -> &Self::Target {
        let len = if self.len == BIG_FENCE_THRESHOLD {
            0
        } else {
            self.len
        };
        &self.fences[..len]
    }
}

/// Shoot down TLBs by broadcasting a [`VmFenceSet`] to all online CPUs.
pub fn shootdown(set: &VmFenceSet) {
    // TODO: Implement when SMP is fixed.
    // Operation: For each online CPU other than the caller, lock and add to its fence set.
    // Then, send an IPI to all cores; they will check and perform the fences described.
    // If IPI is possible, then it is used and waiting is not needed.
    // If IPI is not possible, we wait for the next RCU generation as schedulers will also check it on every context switch.

    set.execute();
}
