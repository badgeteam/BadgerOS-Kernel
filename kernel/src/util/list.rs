// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{marker::PhantomData, ptr::null_mut};

use alloc::sync::Arc;

#[macro_export]
macro_rules! impl_has_list_node {
    ($Type: ty, $field: tt) => {
        impl HasListNode<$Type> for $Type {
            fn list_node(&self) -> &InvasiveListNode<$Type> {
                &self.$field
            }

            fn list_node_mut(&mut self) -> &mut InvasiveListNode<$Type> {
                &mut self.$field
            }

            unsafe fn from_node(node: &InvasiveListNode<$Type>) -> &$Type {
                unsafe {
                    &*((node as *const InvasiveListNode<$Type>).byte_sub(offset_of!($Type, node))
                        as *const $Type)
                }
            }

            unsafe fn from_node_mut(node: &mut InvasiveListNode<$Type>) -> &mut $Type {
                unsafe {
                    &mut *((node as *mut InvasiveListNode<$Type>).byte_sub(offset_of!($Type, node))
                        as *mut $Type)
                }
            }
        }
    };
}

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

/// Invasive linked list iterator.
pub struct InvasiveListIter<'a, T: HasListNode<T>> {
    cur: *mut InvasiveListNode<T>,
    marker: PhantomData<&'a InvasiveList<T>>,
}

impl<'a, T: HasListNode<T>> Iterator for InvasiveListIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        if self.cur < 1 as _ {
            return None;
        }
        unsafe {
            let tmp = T::from_node(&*self.cur);
            self.cur = (*self.cur).next;
            Some(tmp)
        }
    }
}

/// Invasive linked list iterator.
pub struct InvasiveListIterMut<'a, T: HasListNode<T>> {
    cur: *mut InvasiveListNode<T>,
    marker: PhantomData<&'a mut InvasiveList<T>>,
}

impl<'a, T: HasListNode<T>> Iterator for InvasiveListIterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T> {
        if self.cur < 1 as _ {
            return None;
        }
        unsafe {
            let tmp = T::from_node_mut(&mut *self.cur);
            self.cur = (*self.cur).next;
            Some(tmp)
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

    fn consistency_check(&self) {
        #[cfg(debug_assertions)]
        unsafe {
            let mut len = 0usize;
            let mut prev = 1 as _;
            let mut cur = self.first;
            while cur > 1 as _ {
                debug_assert!((*cur).prev == prev, "InvasiveList has broken prev link");
                prev = cur;
                cur = (*cur).next;
                len += 1;
                debug_assert!(len <= self.len, "InvasiveList has too many elements");
            }
            debug_assert!(len == self.len, "InvasiveList has too few elements");
        }
    }

    pub fn push_front<'a>(&'a mut self, item: &'a mut T) -> Result<(), ()> {
        let node = item.list_node_mut();
        if !node.next.is_null() {
            return Err(());
        }
        debug_assert!(node.prev.is_null());

        unsafe {
            node.next = self.first.max(1 as _);
            node.prev = 1 as _;
            if !self.first.is_null() {
                (*self.first).prev = node;
            } else {
                self.last = node;
            }
            self.first = node;
        }

        self.len += 1;
        debug_assert!(self.contains(item));
        self.consistency_check();
        Ok(())
    }

    pub fn pop_front<'a>(&'a mut self) -> Option<&'a mut T> {
        if self.first.is_null() {
            return None;
        }

        let node = self.first;
        unsafe {
            if (*node).next > 1 as _ {
                (*(*node).next).prev = 1 as _;
                self.first = (*node).next;
            } else {
                self.first = null_mut();
                self.last = null_mut();
            }
            *node = InvasiveListNode::new();
        }

        self.len -= 1;
        debug_assert!(!self.contains(unsafe { T::from_node(&*node) }));
        self.consistency_check();
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
            node.prev = self.last.max(1 as _);
            node.next = 1 as _;
            if !self.last.is_null() {
                (*self.last).next = node;
            } else {
                self.first = node;
            }
            self.last = node;
        }

        self.len += 1;
        debug_assert!(self.contains(item));
        self.consistency_check();
        Ok(())
    }

    pub fn pop_back<'a>(&'a mut self) -> Option<&'a mut T> {
        if self.last.is_null() {
            return None;
        }

        let node = self.last;
        unsafe {
            if (*node).prev > 1 as _ {
                (*(*node).prev).next = 1 as _;
                self.last = (*node).prev;
            } else {
                self.first = null_mut();
                self.last = null_mut();
            }
            *node = InvasiveListNode::new();
        }

        self.len -= 1;
        debug_assert!(!self.contains(unsafe { T::from_node(&*node) }));
        self.consistency_check();
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

    pub fn clear(&mut self) {
        let mut cur = self.first;
        self.first = null_mut();
        self.last = null_mut();

        unsafe {
            while cur > 1 as _ {
                let next = (*cur).next;
                (*cur).next = null_mut();
                (*cur).prev = null_mut();
                cur = next;
            }
        }
    }

    pub fn contains(&self, thing: &T) -> bool {
        let node = thing.list_node();
        if node.next.is_null() {
            return false;
        }
        for elem in self.iter() {
            if core::ptr::addr_eq(elem, thing) {
                return true;
            }
        }
        false
    }

    pub fn iter<'a>(&'a self) -> InvasiveListIter<'a, T> {
        InvasiveListIter {
            cur: self.first,
            marker: PhantomData,
        }
    }

    pub fn iter_mut<'a>(&'a mut self) -> InvasiveListIterMut<'a, T> {
        InvasiveListIterMut {
            cur: self.first,
            marker: PhantomData,
        }
    }
}

impl<T: HasListNode<T>> Drop for InvasiveList<T> {
    fn drop(&mut self) {
        self.clear()
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

    pub fn clear(&mut self) {
        let mut cur = self.inner.first;
        self.inner.first = null_mut();
        self.inner.last = null_mut();
        self.inner.len = 0;

        unsafe {
            while cur > 1 as _ {
                let next = (*cur).next;
                (*cur).next = null_mut();
                (*cur).prev = null_mut();
                drop(Arc::from_raw(T::from_node(&*cur)));
                cur = next;
            }
        }
    }

    pub fn contains(&self, thing: &T) -> bool {
        self.inner.contains(thing)
    }

    pub fn iter<'a>(&'a self) -> InvasiveListIter<'a, T> {
        self.inner.iter()
    }

    pub fn iter_mut<'a>(&'a mut self) -> InvasiveListIterMut<'a, T> {
        self.inner.iter_mut()
    }
}

impl<T: HasListNode<T>> Drop for ArcList<T> {
    fn drop(&mut self) {
        self.clear()
    }
}
