// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::ptr::null_mut;

use alloc::sync::Arc;

/// Trait for types that can be stored in an [`InvasiveList`].
pub trait HasListNode<T: HasListNode<T>> {
    unsafe fn from_node(node: &InvasiveListNode<T>) -> &T;
    unsafe fn from_node_mut(node: &mut InvasiveListNode<T>) -> &mut T;
    fn list_node(&self) -> &InvasiveListNode<T>;
    fn list_node_mut(&mut self) -> &mut InvasiveListNode<T>;
}

/// Linked-list node for the [`InvasiveList`].
pub struct InvasiveListNode<T: HasListNode<T>> {
    prev: *mut InvasiveListNode<T>,
    next: *mut InvasiveListNode<T>,
}

impl<T: HasListNode<T>> InvasiveListNode<T> {
    pub const fn new() -> Self {
        Self {
            prev: null_mut(),
            next: null_mut(),
        }
    }
}

/// Invasive linked list.
pub struct InvasiveList<T: HasListNode<T>> {
    first: *mut InvasiveListNode<T>,
    last: *mut InvasiveListNode<T>,
    len: usize,
}

impl<T: HasListNode<T>> InvasiveList<T> {
    pub const fn new() -> Self {
        Self {
            first: null_mut(),
            last: null_mut(),
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn push_front<'a>(&'a mut self, item: &'a mut T) -> Result<(), ()> {
        let node = item.list_node_mut();
        if !node.next.is_null() {
            return Err(());
        }
        debug_assert!(node.prev.is_null());

        unsafe {
            node.next = self.first;
            if !self.first.is_null() {
                (*self.first).prev = node;
            } else {
                self.last = node;
            }
            self.first = node;
        }

        self.len += 1;
        Ok(())
    }

    pub fn pop_front<'a>(&'a mut self) -> Option<&'a mut T> {
        if self.first.is_null() {
            return None;
        }

        let node = self.first;
        unsafe {
            if !(*node).next.is_null() {
                (*(*node).next).prev = null_mut();
            } else {
                self.last = null_mut();
            }
            self.first = (*node).next;
            *node = InvasiveListNode::new();
        }

        self.len -= 1;
        Some(unsafe { T::from_node_mut(&mut *node) })
    }

    pub fn front<'a>(&'a self) -> Option<&'a T> {
        if self.first.is_null() {
            return None;
        }
        Some(unsafe { T::from_node(&*self.first) })
    }

    pub fn front_mut<'a>(&'a mut self) -> Option<&'a T> {
        if self.first.is_null() {
            return None;
        }
        Some(unsafe { T::from_node_mut(&mut *self.first) })
    }

    pub fn push_back<'a>(&'a mut self, item: &'a mut T) -> Result<(), ()> {
        let node = item.list_node_mut();
        if !node.next.is_null() {
            return Err(());
        }
        debug_assert!(node.prev.is_null());

        unsafe {
            node.next = self.last;
            if !self.last.is_null() {
                (*self.last).next = node;
            } else {
                self.first = node;
            }
            self.last = node;
        }

        self.len += 1;
        Ok(())
    }

    pub fn pop_back<'a>(&'a mut self) -> Option<&'a mut T> {
        if self.last.is_null() {
            return None;
        }

        let node = self.last;
        unsafe {
            if !(*node).prev.is_null() {
                (*(*node).prev).next = null_mut();
            } else {
                self.first = null_mut();
            }
            self.last = (*node).prev;
            *node = InvasiveListNode::new();
        }

        self.len -= 1;
        Some(unsafe { T::from_node_mut(&mut *node) })
    }

    pub fn back<'a>(&'a self) -> Option<&'a T> {
        if self.last.is_null() {
            return None;
        }
        Some(unsafe { T::from_node(&*self.last) })
    }

    pub fn back_mut<'a>(&'a mut self) -> Option<&'a T> {
        if self.last.is_null() {
            return None;
        }
        Some(unsafe { T::from_node_mut(&mut *self.last) })
    }
}

/// Invasive linked list for things stored in an [`Arc`].
pub struct ArcList<T: HasListNode<T>> {
    inner: InvasiveList<T>,
}

impl<T: HasListNode<T>> ArcList<T> {
    pub const fn new() -> Self {
        Self {
            inner: InvasiveList::new(),
        }
    }

    pub const fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn push_front(&mut self, item: Arc<T>) -> Result<(), ()> {
        let item = Arc::into_raw(item) as *mut T;
        unsafe {
            let res = self.inner.push_front(&mut *item);
            if res.is_err() {
                drop(Arc::from_raw(item));
            }
            res
        }
    }

    pub fn pop_front(&mut self) -> Option<Arc<T>> {
        self.inner
            .pop_front()
            .map(|raw| unsafe { Arc::from_raw(raw as *const T) })
    }

    pub fn front(&self) -> Option<&T> {
        self.inner.front()
    }

    pub fn push_back(&mut self, item: Arc<T>) -> Result<(), ()> {
        let item = Arc::into_raw(item) as *mut T;
        unsafe {
            let res = self.inner.push_back(&mut *item);
            if res.is_err() {
                drop(Arc::from_raw(item));
            }
            res
        }
    }

    pub fn pop_back(&mut self) -> Option<Arc<T>> {
        self.inner
            .pop_back()
            .map(|raw| unsafe { Arc::from_raw(raw as *const T) })
    }

    pub fn back(&self) -> Option<&T> {
        self.inner.back()
    }
}
