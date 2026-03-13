use core::{marker::PhantomData, mem::offset_of};

use alloc::boxed::Box;

use super::raw::{self, dlist_node_t, dlist_t};

/* ==== Generic doubly-linked list ==== */

pub trait HasDlistNode: Sized {
    fn dlist_node_offset() -> usize;
}
fn t_from_node<T: HasDlistNode>(node: *mut dlist_node_t) -> *mut T {
    (node as usize - T::dlist_node_offset()) as *mut T
}
fn node_from_t<T: HasDlistNode>(t: *mut T) -> *mut dlist_node_t {
    (t as usize + T::dlist_node_offset()) as *mut dlist_node_t
}

macro_rules! impl_has_dlist_node {
    ($for: tt, $nodename: tt) => {
        impl HasDlistNode for $for {
            fn dlist_node_offset() -> usize {
                offset_of!(Self, $nodename)
            }
        }
    };
}

/// Container for items to store in doubly-linked lists.
pub struct DlistNode<T: Sized> {
    inner: raw::dlist_node_t,
    pub data: T,
}

impl<T> HasDlistNode for DlistNode<T> {
    fn dlist_node_offset() -> usize {
        offset_of!(Self, inner)
    }
}

/// Generic iterator for doubly-linked list.
pub struct DlistIterator<'a, T: HasDlistNode> {
    cur: *mut raw::dlist_node_t,
    marker: PhantomData<&'a T>,
}

impl<'a, T: HasDlistNode> Iterator for DlistIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur.is_null() {
            None
        } else {
            unsafe {
                let item = self.cur;
                self.cur = (*self.cur).next;
                Some(&*t_from_node::<T>(item))
            }
        }
    }
}

/// Generic doubly-linked list that manages the memory itself.
pub struct Dlist<T: HasDlistNode> {
    inner: dlist_t,
    marker: PhantomData<T>,
}

impl<T: HasDlistNode> Dlist<T> {
    /// Create a new empty list.
    pub fn new() -> Self {
        Self {
            inner: dlist_t {
                head: 0 as *mut dlist_node_t,
                tail: 0 as *mut dlist_node_t,
                len: 0,
            },
            marker: PhantomData,
        }
    }
    /// Create from a raw `dlist_t`.
    pub unsafe fn from_raw(inner: dlist_t) -> Self {
        Self {
            inner,
            marker: PhantomData,
        }
    }
    /// Convert into a raw `dlist_t`.
    pub unsafe fn into_raw(self) -> dlist_t {
        self.inner
    }
    /// Get the first element of the list.
    pub fn front<'a>(&'a self) -> Option<&'a T> {
        unsafe { t_from_node::<T>(self.inner.head).as_ref() }
    }
    /// Get the last element of the list.
    pub fn back<'a>(&'a self) -> Option<&'a T> {
        unsafe { t_from_node::<T>(self.inner.tail).as_ref() }
    }
    /// Remove the first element from the list.
    pub fn pop_front(&mut self) -> Option<Box<T>> {
        unsafe {
            let node = raw::dlist_pop_front(&raw mut self.inner);
            (!node.is_null()).then(|| Box::from_raw(t_from_node::<T>(node)))
        }
    }
    /// Remove the last element from the list.
    pub fn pop_back(&mut self) -> Option<Box<T>> {
        unsafe {
            let node = raw::dlist_pop_back(&raw mut self.inner);
            (!node.is_null()).then(|| Box::from_raw(t_from_node::<T>(node)))
        }
    }
    /// Get the amount of elements currently in the list.
    pub fn len(&self) -> usize {
        self.inner.len
    }
    /// Clear the list.
    pub fn clear(&mut self) {
        while self.inner.len > 0 {
            self.pop_front();
        }
    }
}

impl<T: HasDlistNode> Drop for Dlist<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<'a, T: HasDlistNode> IntoIterator for &'a Dlist<T> {
    type Item = &'a T;

    type IntoIter = DlistIterator<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        DlistIterator {
            cur: self.inner.head,
            marker: PhantomData,
        }
    }
}
