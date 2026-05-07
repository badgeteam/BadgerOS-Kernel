// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::sync::atomic::AtomicU32;

pub mod pmm;
pub mod vmm;

/// Percentage quota of unused pages to use for caches.
/// If more is used for caches, they will slowly be evicted.
pub static CACHE_QUOTA: AtomicU32 = AtomicU32::new(50);
