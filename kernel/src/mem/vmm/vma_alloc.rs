// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ops::Range;

use alloc::{
    boxed::Box,
    collections::linked_list::{CursorMut, LinkedList},
};

use crate::{
    bindings::{
        error::{EResult, Errno},
        log::LogLevel,
    },
    mem::vmm::VPN,
};

/// Allocator for virtal address ranges.
#[derive(Clone)]
#[repr(C)]
pub struct VmaAlloc {
    free_page_count: VPN,
    /// TODO: Unbox when C no longer depends on the size of this struct.
    free_list: Box<LinkedList<Range<VPN>>>,
}

impl VmaAlloc {
    /// Create an empty allocator.
    pub fn new() -> Self {
        Self {
            free_page_count: 0,
            free_list: Box::new(LinkedList::new()),
        }
    }

    /// The amount of free pages of virtual memory left in total.
    pub fn free_page_count(&self) -> VPN {
        self.free_page_count
    }

    /// Mark a new region as free.
    pub fn free(&mut self, pages: Range<VPN>) {
        if pages.len() == 0 {
            return;
        }

        if self.free_list.is_empty() {
            self.free_page_count += pages.len();
            self.free_list.push_back(pages);
            return;
        }

        // This finds the place to insert and removes overlap from other free regions with this one.
        let mut cursor = self.steal_impl(pages.clone());
        cursor.insert_before(pages.clone());

        // Coalesce the newly inserted entry with its neighbors.
        // The second statement must use `current` because it may no longer equal `pages`.
        if let Some(next) = cursor.peek_next()
            && next.start == pages.end
        {
            next.start = pages.start;
            cursor.remove_current();
        }
        if let Some(prev) = cursor.peek_prev()
            && prev.end == pages.start
        {
            cursor.current().unwrap().start = prev.start;
            cursor.move_prev();
            cursor.remove_current();
        }

        self.free_page_count += pages.len();
    }

    /// Allocate a range of pages.
    pub fn alloc(&mut self, amount: VPN) -> EResult<VPN> {
        if amount == 0 {
            return Err(Errno::EINVAL);
        }

        let mut cursor = self.free_list.cursor_front_mut();
        while let Some(next) = cursor.current() {
            if next.len() == amount {
                let vpn = next.start;
                cursor.remove_current();
                return Ok(vpn);
            } else if next.len() > amount {
                let vpn = next.start;
                next.start += amount;
                return Ok(vpn);
            } else {
                cursor.move_next();
            }
        }

        logkf!(
            LogLevel::Error,
            "Out of virtual memory space (this should be nigh impossible!)"
        );
        Err(Errno::ENOMEM)
    }

    /// Mark a specific range as in use.
    pub fn steal(&mut self, pages: Range<VPN>) {
        if pages.len() == 0 {
            return;
        }
        self.steal_impl(pages);
    }

    /// Common implementation of [`Self::steal`] and [`Self::free`].
    fn steal_impl<'a>(&'a mut self, pages: Range<VPN>) -> CursorMut<'a, Range<usize>> {
        let mut cursor = self.free_list.cursor_front_mut();
        while let Some(elem) = cursor.current() {
            if elem.start >= pages.start && elem.end <= pages.end {
                // Cursor entirely contained within range.
                self.free_page_count -= elem.len();
                cursor.remove_current();
            } else if pages.contains(&elem.end) {
                // End of cursor contained within range.
                self.free_page_count -= elem.end - pages.start;
                elem.end = pages.start;
                cursor.move_next();
            } else if pages.contains(&elem.start) {
                // Start of cursor contained within range.
                self.free_page_count -= pages.end - elem.start;
                elem.start = pages.end;
                break;
            } else if elem.start >= pages.end {
                // First element after range.
                break;
            } else {
                cursor.move_next();
            }
        }
        cursor
    }
}
