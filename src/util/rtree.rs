// Copyright © 2026, __robot@PLT
// SPDX-License-Identifier: MIT

use core::{
    array,
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
    ptr::{NonNull, null_mut},
};

use alloc::{alloc::AllocError, boxed::Box, string::String};
use num::PrimInt;

/// A generic radix tree with an integer key type.
#[derive(Default)]
pub struct RadixTree<K: PrimInt, V: Sized, const L: usize = 4>
where
    [(); 1 << L]:,
{
    root: Option<Box<Node<K, V, L>>>,
    marker: PhantomData<K>,
}

impl<K: PrimInt, V: Sized, const L: usize> RadixTree<K, V, L>
where
    [(); 1 << L]:,
{
    pub const fn new() -> Self {
        Self {
            root: None,
            marker: PhantomData,
        }
    }

    /// Remove all elements from this tree.
    pub fn clear(&mut self) {
        self.root = None;
    }

    /// Get the subkey for a given height.
    #[inline(always)]
    fn subkey(key: K, height: u8) -> usize {
        (key >> (height as usize * L)).to_usize().unwrap() % (1 << L)
    }

    /// Determine whether a key is too large for a certain tree height.
    fn is_too_large(key: K, height: u8) -> bool {
        let type_bits = K::zero().count_zeros();
        let key_bits = type_bits - key.leading_zeros();
        key_bits > height as u32 * L as u32
    }

    /// Determine height needed for a certain key.
    fn key_height(key: K) -> u8 {
        let type_bits = K::zero().count_zeros();
        let key_bits = type_bits - key.leading_zeros();
        key_bits.div_ceil(L as u32).max(1) as u8
    }

    /// Create a tree of a given height.
    fn create_subtree(
        subtree_height: u8,
        key: K,
        value: V,
    ) -> Result<Box<Node<K, V, L>>, AllocError> {
        debug_assert!(subtree_height >= 1);
        let mut cur = MaybeUninit::<Box<Node<K, V, L>>>::uninit();

        let mut value = Some(Box::try_new(value)?);

        for height in 0..subtree_height {
            let subkey = Self::subkey(key, height);

            let mut array;
            if height == 0 {
                array = array::from_fn(|_| NodeValue {
                    data: ManuallyDrop::new(None),
                });
                array[subkey] = NodeValue {
                    data: ManuallyDrop::new(value),
                };
                value = None;
            } else {
                array = array::from_fn(|_| NodeValue {
                    child: ManuallyDrop::new(None),
                });
                array[subkey] = NodeValue {
                    child: ManuallyDrop::new(Some(unsafe { cur.assume_init() })),
                };
            }

            let mut next = Box::try_new(Node {
                parent: null_mut(),
                occupancy: 1,
                height,
                array,
                marker: PhantomData,
            })?;
            if height > 0 {
                unsafe { &mut *next.array[subkey].child }
                    .as_deref_mut()
                    .unwrap()
                    .parent = next.as_mut();
            }

            cur = MaybeUninit::new(next);
        }

        Ok(unsafe { cur.assume_init() })
    }

    /// Insert a value, returning the old one if present.
    pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>, AllocError> {
        if let Some(mut node) = self.root.as_deref_mut() {
            if Self::is_too_large(key, node.height + 1) {
                for height in node.height + 1..Self::key_height(key) {
                    let mut array = array::from_fn(|_| NodeValue {
                        child: ManuallyDrop::new(None),
                    });
                    let tmp = core::mem::take(&mut self.root);
                    array[0] = NodeValue {
                        child: ManuallyDrop::new(tmp),
                    };
                    let mut next = Box::try_new(Node {
                        parent: null_mut(),
                        height,
                        occupancy: 1,
                        array,
                        marker: PhantomData,
                    })?;
                    unsafe { &mut *next.array[0].child }
                        .as_deref_mut()
                        .unwrap()
                        .parent = next.as_mut();
                    self.root = Some(next);
                }

                node = self.root.as_deref_mut().unwrap();
            }

            loop {
                let subkey = Self::subkey(key, node.height);
                if node.height == 0 {
                    let mut old = Some(Box::try_new(value)?);
                    core::mem::swap(&mut old, unsafe { &mut *node.array[subkey].data });
                    if old.is_none() {
                        node.occupancy += 1;
                    }
                    return Ok(old.map(|x| *x));
                }
                let node_ptr = node as *mut _;
                let next = unsafe { &mut *node.array[subkey].child };
                if let Some(next) = next {
                    node = next;
                } else {
                    let mut subtree = Self::create_subtree(Self::key_height(key), key, value)?;
                    subtree.parent = node_ptr;
                    node.occupancy += 1;
                    *next = Some(subtree);
                    return Ok(None);
                }
            }
        } else {
            let subtree = Self::create_subtree(Self::key_height(key), key, value)?;
            self.root = Some(subtree);
            Ok(None)
        }
    }

    /// Garbage-collect a node.
    unsafe fn gc(this: *mut Self, mut node: *mut Node<K, V, L>, key: K) {
        unsafe {
            if (*node).occupancy > 0 {
                return;
            }
            node = (*node).parent;

            while !node.is_null() {
                let subkey = Self::subkey(key, (*node).height);
                (*(*node).array[subkey].child) = None;

                if (*node).occupancy > 0 {
                    return;
                }
            }

            (*this).root = None;
        }
    }

    /// Remove a value, returning it if present.
    pub fn remove(&mut self, key: K) -> Option<V> {
        let this = &raw mut *self;
        let mut cur = self.root.as_deref_mut();

        if let Some(node) = &cur
            && Self::is_too_large(key, node.height + 1)
        {
            return None;
        }

        while let Some(node) = cur {
            let subkey = Self::subkey(key, node.height);
            if node.height == 0 {
                let mut old = None;
                core::mem::swap(&mut old, unsafe { &mut *node.array[subkey].data });
                node.occupancy -= 1;
                unsafe { Self::gc(this, node, key) };
                return old.map(|x| *x);
            } else {
                cur = unsafe { &mut node.array[subkey].child }.as_deref_mut();
            }
        }

        None
    }

    /// Try to look up a value.
    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        let mut cur = self.root.as_deref_mut();

        if let Some(node) = &cur
            && Self::is_too_large(key, node.height + 1)
        {
            return None;
        }

        while let Some(node) = cur {
            let subkey = Self::subkey(key, node.height);
            if node.height == 0 {
                return unsafe { &mut *node.array[subkey].data }.as_deref_mut();
            } else {
                cur = unsafe { &mut *node.array[subkey].child }.as_deref_mut();
            }
        }

        None
    }

    /// Try to look up a value.
    pub fn get(&self, key: K) -> Option<&V> {
        let mut cur = self.root.as_deref();

        if let Some(node) = &cur
            && Self::is_too_large(key, node.height + 1)
        {
            return None;
        }

        while let Some(node) = cur {
            let subkey = Self::subkey(key, node.height);
            if node.height == 0 {
                return unsafe { &*node.array[subkey].data }.as_deref();
            } else {
                cur = unsafe { &*node.array[subkey].child }.as_deref();
            }
        }

        None
    }

    /// Create an iterator over this radix tree.
    pub fn iter(&self) -> RadixTreeIter<'_, K, V, L> {
        RadixTreeIter(unsafe { RadixTreeIterImpl::new(self as *const Self as *mut Self) })
    }

    /// Create an iterator over this radix tree.
    pub fn iter_mut(&mut self) -> RadixTreeIterMut<'_, K, V, L> {
        RadixTreeIterMut(unsafe { RadixTreeIterImpl::new(self) })
    }
}

impl<'a, K: PrimInt, V: Sized, const L: usize> IntoIterator for &'a RadixTree<K, V, L>
where
    [(); 1 << L]:,
{
    type Item = <Self::IntoIter as Iterator>::Item;

    type IntoIter = RadixTreeIter<'a, K, V, L>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K: PrimInt, V: Sized, const L: usize> IntoIterator for &'a mut RadixTree<K, V, L>
where
    [(); 1 << L]:,
{
    type Item = <Self::IntoIter as Iterator>::Item;

    type IntoIter = RadixTreeIterMut<'a, K, V, L>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// A node within a [`RadixTree`].
struct Node<K: PrimInt, V: Sized, const L: usize>
where
    [(); 1 << L]:,
{
    parent: *mut Node<K, V, L>,
    height: u8,
    occupancy: u8,
    array: [NodeValue<K, V, L>; 1 << L],
    marker: PhantomData<K>,
}

impl<K: PrimInt, V: Sized, const L: usize> Drop for Node<K, V, L>
where
    [(); 1 << L]:,
{
    fn drop(&mut self) {
        unsafe {
            if self.height == 0 {
                for i in 0..1 << L {
                    core::ptr::drop_in_place(&raw mut self.array[i].data);
                }
            } else {
                for i in 0..1 << L {
                    core::ptr::drop_in_place(&raw mut self.array[i].child);
                }
            }
        }
    }
}

/// Node child and value union.
union NodeValue<K: PrimInt, V: Sized, const L: usize>
where
    [(); 1 << L]:,
{
    child: ManuallyDrop<Option<Box<Node<K, V, L>>>>,
    data: ManuallyDrop<Option<Box<V>>>,
}

/// Common implementation of [`RadixTreeIter`] and [`RadixTreeIterMut`].
struct RadixTreeIterImpl<'a, K: PrimInt, V: Sized + 'a, const L: usize>
where
    [(); 1 << L]:,
{
    key: Option<K>,
    node: *mut Node<K, V, L>,
    marker: PhantomData<&'a RadixTree<K, V, L>>,
}

impl<'a, K: PrimInt, V: Sized + 'a, const L: usize> RadixTreeIterImpl<'a, K, V, L>
where
    [(); 1 << L]:,
{
    unsafe fn new(tree: *mut RadixTree<K, V, L>) -> Self {
        let node: *mut Node<K, V, L> = unsafe { &mut *tree }
            .root
            .as_deref_mut()
            .map(|x| x as *mut _)
            .unwrap_or(null_mut());
        Self {
            key: (!node.is_null()).then_some(K::zero()),
            node,
            marker: PhantomData,
        }
    }

    /// Traverse downwards to check the current key.
    fn down(&mut self) -> Option<NonNull<V>> {
        let key = self.key?;
        debug_assert!(!self.node.is_null());

        unsafe {
            while (*self.node).height > 0 {
                let subkey = RadixTree::<K, V, L>::subkey(key, (*self.node).height);
                self.node = (&mut *(*self.node).array[subkey].child).as_deref_mut()?;
            }

            let subkey = RadixTree::<K, V, L>::subkey(key, (*self.node).height);
            Some(NonNull::from_mut(
                (&mut *(*self.node).array[subkey].data).as_deref_mut()?,
            ))
        }
    }

    /// Traverse upwards to the nearest parent of the next present key.
    fn advance(&mut self) {
        let key = match self.key {
            Some(x) => x,
            None => return,
        };
        debug_assert!(!self.node.is_null());

        unsafe {
            while !self.node.is_null() {
                let subkey = RadixTree::<K, V, L>::subkey(key, (*self.node).height);
                if subkey != (1 << L) - 1 {
                    let align = K::one() << ((*self.node).height as usize * L);
                    let aligned = key - key % align;
                    self.key = aligned.checked_add(&align);
                    return;
                }
                self.node = (*self.node).parent;
            }
            self.key = None;
        }
    }
}

impl<'a, K: PrimInt, V: Sized + 'a, const L: usize> Iterator for RadixTreeIterImpl<'a, K, V, L>
where
    [(); 1 << L]:,
{
    type Item = (K, NonNull<V>);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(key) = self.key {
            let res = self.down();
            self.advance();
            if let Some(res) = res {
                return Some((key, res));
            }
        }
        None
    }
}

/// Iterator over a [`RadixTree`].
pub struct RadixTreeIter<'a, K: PrimInt, V: Sized + 'a, const L: usize>(
    RadixTreeIterImpl<'a, K, V, L>,
)
where
    [(); 1 << L]:;

impl<'a, K: PrimInt, V: Sized + 'a, const L: usize> Iterator for RadixTreeIter<'a, K, V, L>
where
    [(); 1 << L]:,
{
    type Item = (K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, v)| (k, unsafe { v.as_ref() }))
    }
}

/// Iterator over a [`RadixTree`].
pub struct RadixTreeIterMut<'a, K: PrimInt, V: Sized + 'a, const L: usize>(
    RadixTreeIterImpl<'a, K, V, L>,
)
where
    [(); 1 << L]:;

impl<'a, K: PrimInt, V: Sized + 'a, const L: usize> Iterator for RadixTreeIterMut<'a, K, V, L>
where
    [(); 1 << L]:,
{
    type Item = (K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, mut v)| (k, unsafe { v.as_mut() }))
    }
}

heap_ktest! {RTREE_BASIC,
    let mut tree = RadixTree::<u32, String>::new();

    ktest_assert!(tree.insert(0, "Initial".into())?.is_none());
    ktest_assert!(tree.get(0).map(|x| *x == "Initial").unwrap_or(false));

    ktest_assert!(tree.insert(1024, "Center".into())?.is_none());
    ktest_assert!(tree.get(1024).map(|x| *x == "Center").unwrap_or(false));

    ktest_assert!(tree.insert(u32::MAX, "Final".into())?.is_none());
    ktest_assert!(tree.get(u32::MAX).map(|x| *x == "Final").unwrap_or(false));
}

heap_ktest! {RTREE_FILL,
    let mut tree = RadixTree::<u32, u32>::new();

    for i in 0..8192 {
        tree.insert(i, i)?;
    }
    for i in 0..8192 {
        ktest_assert!(tree.get(i) == Some(&i));
    }
}

heap_ktest! {RTREE_ITER,
    let mut tree = RadixTree::<u32, u32>::new();

    tree.insert(0, 123)?;
    tree.insert(2, 456)?;
    tree.insert(17, 789)?;
    tree.insert(31, 31000)?;
    tree.insert(32, 32000)?;
    tree.insert(32768, 1337)?;
    tree.insert(u32::MAX, 42)?;

    let mut iter = tree.iter();
    ktest_expect!(iter.next(), Some((0, &123)));
    ktest_expect!(iter.next(), Some((2, &456)));
    ktest_expect!(iter.next(), Some((17, &789)));
    ktest_expect!(iter.next(), Some((31, &31000)));
    ktest_expect!(iter.next(), Some((32, &32000)));
    ktest_expect!(iter.next(), Some((32768, &1337)));
    ktest_expect!(iter.next(), Some((u32::MAX, &42)));
    ktest_expect!(iter.next(), None);
}
