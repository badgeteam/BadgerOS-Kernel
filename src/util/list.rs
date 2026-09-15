// Copyright © 2026, __robot@PLT
// SPDX-License-Identifier: MIT

use core::{marker::PhantomData, ptr::null_mut};

use alloc::{
    alloc::{Allocator, Global},
    boxed::Box,
    sync::Arc,
};

#[cfg(feature = "dlist_debug")]
macro_rules! dlist_debug_assert {
    ($($x: tt)*) => {
        assert!($($x)*);
    };
}

#[cfg(not(feature = "dlist_debug"))]
macro_rules! dlist_debug_assert {
    ($($_: tt)*) => {};
}

#[macro_export]
macro_rules! impl_has_list_node {
    ($Type: ty, $($field: tt)+) => {
        impl crate::util::list::HasListNode<$Type> for $Type {
            unsafe fn from_node(node: *mut crate::util::list::IntrusiveListNode) -> *mut $Type {
                unsafe { node.byte_sub(core::mem::offset_of!($Type, $($field)+)) as *mut $Type }
            }

            fn list_node(&self) -> &crate::util::list::IntrusiveListNode {
                &self.$($field)+
            }

            fn list_node_mut(&mut self) -> &mut crate::util::list::IntrusiveListNode {
                &mut self.$($field)+
            }
        }
    };
}

/// Trait for types that can be stored in an [`IntrusiveList`].
pub trait HasListNode<T: HasListNode<T>> {
    unsafe fn from_node(node: *mut IntrusiveListNode) -> *mut T;
    fn list_node(&self) -> &IntrusiveListNode;
    fn list_node_mut(&mut self) -> &mut IntrusiveListNode;
}

impl HasListNode<IntrusiveListNode> for IntrusiveListNode {
    unsafe fn from_node(node: *mut IntrusiveListNode) -> *mut IntrusiveListNode {
        node
    }

    fn list_node(&self) -> &IntrusiveListNode {
        self
    }

    fn list_node_mut(&mut self) -> &mut IntrusiveListNode {
        self
    }
}

/// Linked-list node for the [`IntrusiveList`].
#[repr(C)]
pub struct IntrusiveListNode {
    prev: *mut IntrusiveListNode,
    next: *mut IntrusiveListNode,
}

impl IntrusiveListNode {
    pub const fn new() -> Self {
        Self {
            prev: null_mut(),
            next: null_mut(),
        }
    }

    pub fn is_in_list(&self) -> bool {
        self.prev != null_mut()
    }
}

/// Invasive linked list iterator.
pub struct IntrusiveListIter<'a, T: HasListNode<T>> {
    cur: *mut IntrusiveListNode,
    marker: PhantomData<&'a IntrusiveList<T>>,
}

impl<'a, T: HasListNode<T>> Iterator for IntrusiveListIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        if self.cur <= 1 as _ {
            return None;
        }
        unsafe {
            let tmp = T::from_node(self.cur);
            self.cur = (*self.cur).next;
            Some(&*tmp)
        }
    }
}

/// Invasive linked list.
#[repr(C)]
pub struct IntrusiveList<T: HasListNode<T>> {
    first: *mut IntrusiveListNode,
    last: *mut IntrusiveListNode,
    len: usize,
    marker: PhantomData<*mut T>,
}

impl<T: HasListNode<T>> Default for IntrusiveList<T> {
    fn default() -> Self {
        Self {
            first: null_mut(),
            last: null_mut(),
            len: 0,
            marker: PhantomData,
        }
    }
}

impl<T: HasListNode<T>> IntrusiveList<T> {
    pub const fn new() -> Self {
        Self {
            first: null_mut(),
            last: null_mut(),
            len: 0,
            marker: PhantomData,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    fn consistency_check(&self) {
        #[cfg(feature = "dlist_debug")]
        unsafe {
            let mut len = 0usize;
            let mut prev = 1 as _;
            let mut cur = self.first;
            while cur > 1 as _ {
                dlist_debug_assert!((*cur).prev == prev, "IntrusiveList has broken prev link");
                prev = cur;
                cur = (*cur).next;
                len += 1;
                dlist_debug_assert!(len <= self.len, "IntrusiveList has too many elements");
            }
            dlist_debug_assert!(len == self.len, "IntrusiveList has too few elements");
        }
    }

    pub unsafe fn push_front(&mut self, item: *mut T) -> Result<(), ()> {
        let node = unsafe { &mut *item }.list_node_mut();
        if !node.next.is_null() {
            return Err(());
        }
        dlist_debug_assert!(node.prev.is_null());

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
        dlist_debug_assert!(self.contains(item));
        self.consistency_check();
        Ok(())
    }

    pub unsafe fn pop_front(&mut self) -> Option<*mut T> {
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
            *node = IntrusiveListNode::new();
        }

        self.len -= 1;
        dlist_debug_assert!(!self.contains(unsafe { T::from_node(node) }));
        self.consistency_check();
        Some(unsafe { T::from_node(node) })
    }

    pub fn front(&self) -> Option<*mut T> {
        if self.first.is_null() {
            return None;
        }
        Some(unsafe { T::from_node(self.first) })
    }

    pub unsafe fn push_back(&mut self, item: *mut T) -> Result<(), ()> {
        let node = unsafe { &mut *item }.list_node_mut();
        if !node.next.is_null() {
            return Err(());
        }
        dlist_debug_assert!(node.prev.is_null());

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
        dlist_debug_assert!(self.contains(item));
        self.consistency_check();
        Ok(())
    }

    pub unsafe fn pop_back(&mut self) -> Option<*mut T> {
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
            *node = IntrusiveListNode::new();
        }

        self.len -= 1;
        dlist_debug_assert!(!self.contains(unsafe { T::from_node(node) }));
        self.consistency_check();
        Some(unsafe { T::from_node(node) })
    }

    pub fn back(&self) -> Option<*mut T> {
        if self.last.is_null() {
            return None;
        }
        Some(unsafe { T::from_node(self.last) })
    }

    pub unsafe fn insert_after(&mut self, at: *mut T, insert: *mut T) {
        let at_node = unsafe { &mut *at }.list_node_mut();
        let ins_node = unsafe { &mut *insert }.list_node_mut();
        dlist_debug_assert!(self.contains(at));
        dlist_debug_assert!(!self.contains(insert));

        unsafe {
            if at_node.next > 1 as _ {
                (*at_node.next).prev = ins_node;
            } else {
                self.last = ins_node;
            }
            ins_node.prev = at_node;
            ins_node.next = at_node.next;
            at_node.next = ins_node;
        }

        self.len += 1;
    }

    pub unsafe fn insert_before(&mut self, at: *mut T, insert: *mut T) {
        let at_node = unsafe { &mut *at }.list_node_mut();
        let ins_node = unsafe { &mut *insert }.list_node_mut();
        dlist_debug_assert!(self.contains(at));
        dlist_debug_assert!(!self.contains(insert));

        unsafe {
            if at_node.prev > 1 as _ {
                (*at_node.prev).next = ins_node;
            } else {
                self.first = ins_node;
            }
            ins_node.next = at_node;
            ins_node.prev = at_node.prev;
            at_node.prev = ins_node;
        }

        self.len += 1;
    }

    pub unsafe fn next(&self, item: *mut T) -> Option<*mut T> {
        let node = unsafe { &mut *item }.list_node_mut();
        dlist_debug_assert!(self.contains(at));

        unsafe {
            if node.next > 1 as _ {
                Some(T::from_node(node.next))
            } else {
                None
            }
        }
    }

    pub unsafe fn prev(&self, item: *mut T) -> Option<*mut T> {
        let node = unsafe { &mut *item }.list_node_mut();
        dlist_debug_assert!(self.contains(at));

        unsafe {
            if node.prev > 1 as _ {
                Some(T::from_node(node.prev))
            } else {
                None
            }
        }
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

    pub fn contains(&self, thing: *const T) -> bool {
        let node = unsafe { &*thing }.list_node();
        if node.next.is_null() {
            return false;
        }
        for elem in unsafe { self.iter() } {
            if core::ptr::addr_eq(elem, thing) {
                return true;
            }
        }
        false
    }

    pub unsafe fn try_remove(&mut self, item: *mut T) {
        unsafe {
            if (&*item).list_node().is_in_list() {
                self.remove(item);
            }
        }
    }

    pub unsafe fn remove(&mut self, item: *mut T) {
        let node = unsafe { &mut *item }.list_node_mut();
        dlist_debug_assert!(self.contains(item));

        unsafe {
            if node.next > 1 as _ {
                (*node.next).prev = node.prev;
            } else if node.prev == 1 as _ {
                self.last = null_mut();
            } else {
                self.last = node.prev;
            }

            if node.prev > 1 as _ {
                (*node.prev).next = node.next;
            } else if node.next == 1 as _ {
                self.first = null_mut();
            } else {
                self.first = node.next;
            }
        }

        *node = IntrusiveListNode::new();
        self.len -= 1;

        dlist_debug_assert!(!self.contains(item));
        self.consistency_check();
    }

    pub unsafe fn iter<'a>(&'a self) -> IntrusiveListIter<'a, T> {
        IntrusiveListIter {
            cur: self.first,
            marker: PhantomData,
        }
    }
}

impl<T: HasListNode<T>> Drop for IntrusiveList<T> {
    fn drop(&mut self) {
        self.clear()
    }
}

pub trait IntrusiveListAlloc: Allocator + Copy {}
impl<T> IntrusiveListAlloc for T where T: Allocator + Copy {}

/// Invasive linked list for things stored in an [`Arc`].
pub struct ArcIntrusiveList<T: HasListNode<T>, A: IntrusiveListAlloc = Global> {
    pub inner: IntrusiveList<T>,
    alloc: A,
}

impl<T: HasListNode<T>> ArcIntrusiveList<T, Global> {
    pub const fn new() -> Self {
        Self {
            inner: IntrusiveList::new(),
            alloc: Global,
        }
    }
}

impl<T: HasListNode<T>, A: IntrusiveListAlloc> ArcIntrusiveList<T, A> {
    pub const fn new_in(alloc: A) -> Self {
        Self {
            inner: IntrusiveList::new(),
            alloc,
        }
    }

    pub const fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn push_front(&mut self, item: Arc<T, A>) -> Result<(), ()> {
        let item = Arc::into_raw_with_allocator(item).0 as *mut T;
        unsafe {
            let res = self.inner.push_front(&mut *item);
            if res.is_err() {
                drop(Arc::from_raw(item));
            }
            res
        }
    }

    pub fn pop_front(&mut self) -> Option<Arc<T, A>> {
        unsafe { self.inner.pop_front() }
            .map(|raw| unsafe { Arc::from_raw_in(raw as *const T, self.alloc) })
    }

    pub fn front(&self) -> Option<&T> {
        self.inner.front().map(|x| unsafe { &*x })
    }

    pub fn push_back(&mut self, item: Arc<T, A>) -> Result<(), ()> {
        let item = Arc::into_raw_with_allocator(item).0 as *mut T;
        unsafe {
            let res = self.inner.push_back(&mut *item);
            if res.is_err() {
                drop(Arc::from_raw(item));
            }
            res
        }
    }

    pub fn pop_back(&mut self) -> Option<Arc<T, A>> {
        unsafe { self.inner.pop_back() }
            .map(|raw| unsafe { Arc::from_raw_in(raw as *const T, self.alloc) })
    }

    pub fn back(&self) -> Option<&T> {
        self.inner.back().map(|x| unsafe { &*x })
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
                drop(Arc::from_raw(T::from_node(cur)));
                cur = next;
            }
        }
    }

    pub fn contains(&self, thing: &T) -> bool {
        self.inner.contains(thing)
    }

    pub fn iter<'a>(&'a self) -> IntrusiveListIter<'a, T> {
        unsafe { self.inner.iter() }
    }
}

impl<T: HasListNode<T>, A: IntrusiveListAlloc> Drop for ArcIntrusiveList<T, A> {
    fn drop(&mut self) {
        self.clear()
    }
}

/// Invasive linked list for things stored in an [`Arc`].
pub struct BoxIntrusiveList<T: HasListNode<T>, A: IntrusiveListAlloc = Global> {
    pub inner: IntrusiveList<T>,
    alloc: A,
}

impl<T: HasListNode<T>> BoxIntrusiveList<T, Global> {
    pub const fn new() -> Self {
        Self {
            inner: IntrusiveList::new(),
            alloc: Global,
        }
    }
}

impl<T: HasListNode<T>, A: IntrusiveListAlloc> BoxIntrusiveList<T, A> {
    pub const fn new_in(alloc: A) -> Self {
        Self {
            inner: IntrusiveList::new(),
            alloc,
        }
    }

    pub const fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn push_front(&mut self, item: Box<T, A>) {
        let item = Box::into_raw_with_allocator(item).0 as *mut T;
        unsafe {
            self.inner
                .push_front(item)
                .expect("Item already in list, but Box has exclusive ownership");
        }
    }

    pub fn pop_front(&mut self) -> Option<Box<T, A>> {
        unsafe { self.inner.pop_front() }
            .map(|raw| unsafe { Box::from_raw_in(raw as *mut T, self.alloc) })
    }

    pub fn front(&self) -> Option<&T> {
        self.inner.front().map(|x| unsafe { &*x })
    }

    pub fn front_mut(&mut self) -> Option<&mut T> {
        self.inner.front().map(|x| unsafe { &mut *x })
    }

    pub fn push_back(&mut self, item: Box<T, A>) {
        let item = Box::into_raw_with_allocator(item).0 as *mut T;
        unsafe {
            self.inner
                .push_back(item)
                .expect("Item already in list, but Box has exclusive ownership");
        }
    }

    pub fn pop_back(&mut self) -> Option<Box<T, A>> {
        unsafe { self.inner.pop_back() }
            .map(|raw| unsafe { Box::from_raw_in(raw as *mut T, self.alloc) })
    }

    pub fn back(&self) -> Option<&T> {
        self.inner.back().map(|x| unsafe { &*x })
    }

    pub fn back_mut(&mut self) -> Option<&mut T> {
        self.inner.back().map(|x| unsafe { &mut *x })
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
                drop(Box::from_raw_in(T::from_node(cur), self.alloc));
                cur = next;
            }
        }
    }

    pub fn contains(&self, thing: &T) -> bool {
        self.inner.contains(thing)
    }

    pub fn iter<'a>(&'a self) -> IntrusiveListIter<'a, T> {
        unsafe { self.inner.iter() }
    }
}

impl<T: HasListNode<T>, A: IntrusiveListAlloc> Drop for BoxIntrusiveList<T, A> {
    fn drop(&mut self) {
        self.clear()
    }
}
