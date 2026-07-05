use core::{
    error::Error,
    fmt::{Debug, Display},
    num::NonZeroU32,
};

use alloc::{alloc::AllocError, collections::TryReserveError, vec::Vec};

use crate::{LogLevel, bindings::error::Errno};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdAllocError {
    NoId,
    NoMem,
}

impl Display for IdAllocError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoId => f.write_str("ID allocator exhausted"),
            Self::NoMem => f.write_str("Out of memory"),
        }
    }
}

impl Error for IdAllocError {}

impl From<TryReserveError> for IdAllocError {
    fn from(_: TryReserveError) -> Self {
        Self::NoMem
    }
}

impl From<AllocError> for IdAllocError {
    fn from(_: AllocError) -> Self {
        Self::NoMem
    }
}

impl From<IdAllocError> for Errno {
    fn from(_: IdAllocError) -> Self {
        Errno::ENOMEM
    }
}

/// Inclusive allocated range entry of [`IdAlloc`].
#[derive(Clone, Copy)]
struct Entry {
    first: NonZeroU32,
    last: NonZeroU32,
}

/// Generic 32-bit non-zero ID allocator.
pub struct IdAlloc {
    next: NonZeroU32,
    avail: Vec<Entry>,
}

impl IdAlloc {
    pub fn new() -> Result<Self, TryReserveError> {
        let mut avail = Vec::try_with_capacity(1)?;
        avail.push(Entry {
            first: NonZeroU32::new(1).unwrap(),
            last: NonZeroU32::new(u32::MAX).unwrap(),
        });
        Ok(Self {
            next: NonZeroU32::new(1).unwrap(),
            avail,
        })
    }

    fn update_next(id: NonZeroU32) -> NonZeroU32 {
        NonZeroU32::new(id.get().wrapping_add(1)).unwrap_or(NonZeroU32::new(1).unwrap())
    }

    pub fn dealloc(&mut self, id: NonZeroU32) {
        let index = match self.avail.binary_search_by(|entry| entry.first.cmp(&id)) {
            Ok(_) => {
                logkf!(LogLevel::Warning, "Double free of id {}", id);
                return;
            }
            Err(index) => index,
        };

        let merge_left = index > 0
            && self.avail[index - 1]
                .last
                .get()
                .checked_add(1)
                .is_some_and(|next| next == id.get());
        let merge_right = index < self.avail.len()
            && id
                .get()
                .checked_add(1)
                .is_some_and(|next| next == self.avail[index].first.get());

        if merge_left && merge_right {
            self.avail[index - 1].last = self.avail[index].last;
            self.avail.remove(index);
        } else if merge_left {
            self.avail[index - 1].last = id;
        } else if merge_right {
            self.avail[index].first = id;
        } else if self.avail.try_reserve(1).is_ok() {
            self.avail.insert(
                index,
                Entry {
                    first: id,
                    last: id,
                },
            );
        }
    }

    pub fn alloc(&mut self) -> Result<NonZeroU32, IdAllocError> {
        let mut id = self.next;
        if self.avail.len() == 0 {
            return Err(IdAllocError::NoId);
        }
        let mut index = match self.avail.binary_search_by(|f| f.first.cmp(&id)) {
            Ok(index) => index,
            Err(index) => {
                if index < self.avail.len() {
                    index
                } else {
                    id = NonZeroU32::new(1).unwrap();
                    0
                }
            }
        };
        let mut entry = self.avail[index];
        if id < entry.first {
            id = entry.first;
        } else if id > entry.last {
            index += 1;
            if let Some(&x) = self.avail.get(index) {
                entry = x;
                id = entry.first;
            } else {
                return Err(IdAllocError::NoId);
            }
        }

        if id == entry.first && id == entry.last {
            self.avail.remove(index);
        } else if id == entry.first {
            // SAFETY: `id != entry.last` means `entry.last > entry.first`, so this can't overflow.
            self.avail[index].first = unsafe { NonZeroU32::new_unchecked(entry.first.get() + 1) };
        } else if id == entry.last {
            // SAFETY: `id != entry.first` means `entry.first < entry.last`, so this can't underlow.
            self.avail[index].last = unsafe { NonZeroU32::new_unchecked(entry.last.get() - 1) };
        } else {
            if id > entry.last {
                return Err(IdAllocError::NoId);
            }
            self.avail.try_reserve(1)?;
            let entry2 = Entry {
                // SAFETY: `entry.first < id < entry.last`, so this can't overflow.
                first: unsafe { NonZeroU32::new_unchecked(id.get() + 1) },
                last: entry.last,
            };
            // SAFETY: `entry.first < id < entry.last`, so this can't underflow.
            self.avail[index].last = unsafe { NonZeroU32::new_unchecked(entry.last.get() - 1) };
            self.avail.insert(index + 1, entry2);
        }

        self.next = Self::update_next(id);

        Ok(id)
    }

    pub fn is_free(&self, id: NonZeroU32) -> bool {
        self.avail
            .binary_search_by(|entry| {
                if id < entry.first {
                    core::cmp::Ordering::Greater
                } else if id > entry.last {
                    core::cmp::Ordering::Less
                } else {
                    core::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }
}

macro_rules! alloc_for_test {
    ($alloc:expr , $expect:expr) => {
        ktest_expect!($alloc.alloc()?.get(), $expect);
        ktest_assert!(!$alloc.is_free(NonZeroU32::new($expect).unwrap()));
        ktest_assert!($alloc.avail.is_sorted_by(|a, b| a.first < b.first));
    };
}

macro_rules! dealloc_for_test {
    ($slloc:expr , $value:expr) => {
        alloc.dealloc(NonZeroU32::new($value).unwrap());
        ktest_assert!($alloc.is_free(NonZeroU32::new($value).unwrap()));
        ktest_assert!($alloc.avail.is_sorted_by(|a, b| a.first < b.first));
    };
}

heap_ktest! { ID_ALLOC_SIMPLE,
    let mut alloc = IdAlloc::new()?;

    alloc_for_test!(alloc, 1);
    alloc_for_test!(alloc, 2);
    alloc_for_test!(alloc, 3);
    alloc_for_test!(alloc, 4);

    alloc.dealloc(NonZeroU32::new(1).unwrap());
    alloc.dealloc(NonZeroU32::new(3).unwrap());
    alloc.dealloc(NonZeroU32::new(2).unwrap());
    alloc.dealloc(NonZeroU32::new(4).unwrap());

    ktest_expect!(alloc.avail.len(), 1);
    ktest_expect!(alloc.avail[0].first.get(), 1);
    ktest_expect!(alloc.avail[0].last.get(), u32::MAX);
}

heap_ktest! { ID_ALLOC_SCRAMBLE,
    let mut alloc = IdAlloc::new()?;

    for i in 1..=8 {
        alloc_for_test!(alloc, i);
    }

    alloc.dealloc(NonZeroU32::new(1).unwrap());
    alloc.dealloc(NonZeroU32::new(2).unwrap());
    alloc.dealloc(NonZeroU32::new(4).unwrap());
    alloc.dealloc(NonZeroU32::new(3).unwrap());
    alloc.dealloc(NonZeroU32::new(5).unwrap());
    alloc.dealloc(NonZeroU32::new(8).unwrap());
    alloc.dealloc(NonZeroU32::new(6).unwrap());
    alloc.dealloc(NonZeroU32::new(7).unwrap());


    ktest_expect!(alloc.avail.len(), 1);
    ktest_expect!(alloc.avail[0].first.get(), 1);
    ktest_expect!(alloc.avail[0].last.get(), u32::MAX);

    // We're going to test wrapping, so remove all but the first 64 IDs.
    alloc.avail[0].last = NonZeroU32::new(64).unwrap();

    for i in 1..=64 {
        alloc_for_test!(alloc, i);
    }

    alloc.dealloc(NonZeroU32::new(8).unwrap());
    alloc.dealloc(NonZeroU32::new(26).unwrap());
    alloc.dealloc(NonZeroU32::new(54).unwrap());
    alloc.dealloc(NonZeroU32::new(41).unwrap());
    alloc.dealloc(NonZeroU32::new(56).unwrap());
    alloc.dealloc(NonZeroU32::new(15).unwrap());
    alloc.dealloc(NonZeroU32::new(3).unwrap());
    alloc.dealloc(NonZeroU32::new(27).unwrap());

    alloc.next = NonZeroU32::new(28).unwrap();

    alloc_for_test!(alloc, 41);
    alloc_for_test!(alloc, 54);
    alloc_for_test!(alloc, 56);
    // `next` overflows here back to 1
    alloc_for_test!(alloc, 3);
    alloc_for_test!(alloc, 8);
    alloc_for_test!(alloc, 15);
    alloc_for_test!(alloc, 26);
    alloc_for_test!(alloc, 27);
}

heap_ktest! { ID_ALLOC_MANY,
    let mut alloc = IdAlloc::new()?;

    for i in 1..10000 {
        alloc_for_test!(alloc, i);
    }
    for i in 1..10000 {
        alloc.dealloc(NonZeroU32::new(i).unwrap());
    }

    ktest_expect!(alloc.avail.len(), 1);
    ktest_expect!(alloc.avail[0].first.get(), 1);
    ktest_expect!(alloc.avail[0].last.get(), u32::MAX);
}
