// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use alloc::sync::Arc;

use crate::{
    kernel::sched::{SchedAlgorithm, Thread},
    util::list::ArcList,
};

use super::*;

/// A simple round-robin scheduler.
pub struct RoundRobinAlgorithm {
    queue: ArcList<Thread>,
}

impl RoundRobinAlgorithm {
    pub const fn new() -> Self {
        Self {
            queue: ArcList::new(),
        }
    }
}

impl SchedAlgorithm for RoundRobinAlgorithm {
    fn return_thread(&mut self, thread: Arc<Thread>) {
        self.queue.push_back(thread).unwrap();
    }

    fn remove_thread(&mut self, thread: Arc<Thread>) {
        todo!()
    }

    fn add_thread(&mut self, thread: Arc<Thread>) {
        self.queue.push_back(thread).unwrap();
    }

    fn add_thread_front(&mut self, thread: Arc<Thread>) {
        self.queue.push_front(thread).unwrap();
    }

    fn choose_thread(&mut self) -> Option<Arc<Thread>> {
        for _ in 0..self.queue.len() {
            let node = self.queue.pop_front().unwrap();
            let flags = node.flags.load(Ordering::Relaxed);

            if flags & tflags::STOPPED != 0 {
                ZOMBIES.lock().push_back(node).unwrap();
                REAPER
                    .lock_shared()
                    .as_deref()
                    .unwrap()
                    .flags
                    .fetch_and(!tflags::BLOCKED, Ordering::Relaxed);
                continue;
            } else if flags & tflags::BLOCKED == 0 {
                return Some(node);
            }

            self.queue.push_back(node).unwrap();
        }
        None
    }

    fn len(&self) -> usize {
        self.queue.len()
    }
}
