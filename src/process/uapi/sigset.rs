// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#![allow(non_camel_case_types)]

use crate::util::bitset::BitSet;

pub type sigset_t = BitSet<1024>;
